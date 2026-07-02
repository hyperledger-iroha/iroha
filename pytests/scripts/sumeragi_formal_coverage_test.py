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


def test_formal_readme_guard_contract_snippets_pin_namespace_docs(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    readme = tmp_path / "README.md"
    readme.write_text(
        "Constants and variables share a single TLA declaration namespace\n",
        encoding="utf-8",
    )

    assert module.required_text_errors(
        readme,
        module.FORMAL_README_GUARD_CONTRACT_SNIPPETS,
        "Sumeragi formal README",
    ) == [
        f"Sumeragi formal README {readme} is missing required text: "
        "Declared constants and variables must also remain",
        f"Sumeragi formal README {readme} is missing required text: "
        "disjoint from top-level operator definitions and `RECURSIVE` declarations",
        f"Sumeragi formal README {readme} is missing required text: "
        "same operator name must not be reused across behavior",
        f"Sumeragi formal README {readme} is missing required text: "
        "constraint, and proof-check roles",
        f"Sumeragi formal README {readme} is missing required text: "
        "TLA operator definitions must be non-LOCAL",
        f"Sumeragi formal README {readme} is missing required text: "
        "TLA `RECURSIVE` declaration directives must be top-level",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed `RECURSIVE` starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level no-separator `RECURSIVE` starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "aliases must be duplicate-free",
        f"Sumeragi formal README {readme} is missing required text: "
        "top-level proof-target operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "be duplicate-free, use non-reserved static module identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "be top-level",
        f"Sumeragi formal README {readme} is missing required text: "
        "appear before declarations and definitions",
        f"Sumeragi formal README {readme} is missing required text: "
        "without `WITH` substitutions",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed `EXTENDS`/`INSTANCE` starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "No-separator `EXTENDS`/`INSTANCE` starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed named `INSTANCE` aliases are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "No-separator named `INSTANCE` aliases are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "INSTANCE declarations must be non-LOCAL",
        f"Sumeragi formal README {readme} is missing required text: "
        "Local TLA dependency files are followed transitively",
        f"Sumeragi formal README {readme} is missing required text: "
        "same module-header, declaration, and assumption/proof guards",
        f"Sumeragi formal README {readme} is missing required text: "
        "Assumption/proof directive starts are rejected even when indented",
        f"Sumeragi formal README {readme} is missing required text: "
        "No-separator assumption/proof directive starts are rejected even when indented",
        f"Sumeragi formal README {readme} is missing required text: "
        "TLA module headers and terminators must be top-level",
        f"Sumeragi formal README {readme} is missing required text: "
        "Decorative all-`=` separator lines are allowed before that terminator",
        f"Sumeragi formal README {readme} is missing required text: "
        "Decorative all-`=` separator lines must not have trailing content",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed TLA module header starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "No-separator TLA module header starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed TLA terminator starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "TLA constant and variable declaration directives must be top-level",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed TLA constant/variable declaration starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level no-separator TLA constant/variable declaration starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level no-separator TLA declaration block entries are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed TLA `vars` tuple starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Directive-prefixed TLA declaration block entries remain valid",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed supported CFG directive starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Directive-prefixed CFG block entries remain valid",
        f"Sumeragi formal README {readme} is missing required text: "
        "Indented no-separator supported CFG directive starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed CHECK_DEADLOCK starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed CFG constant binding starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level no-separator CFG constant binding starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Indented no-separator CFG constant binding directive starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Malformed CFG operator-reference directive starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level no-separator CFG operator-reference directive starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "Indented no-separator CFG operator-reference directive starts are rejected",
        f"Sumeragi formal README {readme} is missing required text: "
        "control-flow, implication, or equivalence exactness definitions must name",
        f"Sumeragi formal README {readme} is missing required text: "
        "conjuncts must be named concrete predicates before composition",
        f"Sumeragi formal README {readme} is missing required text: "
        "conjuncts must compose an existing concrete matches predicate directly",
        f"Sumeragi formal README {readme} is missing required text: "
        "compose named predicates before the exactness bundle composes them",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized exactness conjuncts must be lifted behind zero-arity",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized helper call checks parse expression arguments, including comparisons",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound exactness helper operands must not hide expression-argument parameterized helper calls",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified formula exactness conjuncts must be lifted behind named",
        f"Sumeragi formal README {readme} is missing required text: "
        "Formula equality exactness conjuncts must be lifted behind named",
        f"Sumeragi formal README {readme} is missing required text: "
        "Formula equivalence exactness conjuncts must be lifted behind named",
        f"Sumeragi formal README {readme} is missing required text: "
        "Non-named exactness conjuncts are rejected even when mixed",
        f"Sumeragi formal README {readme} is missing required text: "
        "Named exactness predicates must not hide generic correctness",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive named exactness predicate chains must not hide generic correctness",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide repeated helper conjuncts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal exactness helper wrappers must not hide repeated helper conjuncts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal exactness helper wrappers must not hide single-helper conjunct aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated exactness helper wrappers must not hide single-helper conjunct aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated exactness helper wrappers must not hide zero-arity helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated zero-arity helper alias checks recurse through nested identity gates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated exactness helper wrappers must not hide negated helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated negated helper operand checks recurse through nested identity gates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound exactness helper operands must not hide repeated helper conjuncts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Helper conjunct repetition checks traverse unary-temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Repeated helper same-polarity checks split top-level boolean operands before peeling temporal or negated wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide repeated helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Helper operand repetition checks traverse unary-temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Repeated helper operand checks include chained implication and equivalence operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide contradictory helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide excluded-middle helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide complementary-equivalence helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Complementary-equivalence checks include chained equivalence operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Helper operand polarity checks traverse unary-temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Helper operand polarity checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide undefined helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas must not hide undefined helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve quantified binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve unbounded quantified binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans reject relation-bearing quantified binding prefixes",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve tuple-pattern quantifier domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve LET binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve parameterized LET operator scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve CHOOSE binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve LAMBDA binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans reject relation-bearing CHOOSE/LAMBDA binding prefixes",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve standard TLA set/operator identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve ENABLED/UNCHANGED operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve CASE branch scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve relation operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve operator-call argument scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve arithmetic/set infix operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve sequence/function infix operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve explicit set literal element scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve unary set-operator operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve set-comprehension binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve set-comprehension outer enclosure scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve function-constructor binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve function-set domain and range scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Function-set scans preserve CASE domain branch arrows",
        f"Sumeragi formal README {readme} is missing required text: "
        "Function-set scans preserve record maplet CASE values",
        f"Sumeragi formal README {readme} is missing required text: "
        "Function-set scans preserve record set/update CASE values",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record set field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record update field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record selector field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve comma-shared set/function binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve operator parameter scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas must not be vacuous",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper formulas must not restate empty-domain, singleton-domain, bound-domain, self-membership, or empty-set membership facts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper restatement checks reject pure top-level boolean compositions",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper restatement checks reject identity-literal gates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper restatement checks propagate known truth values",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified formula prefix scans preserve escaped string literal colons",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified formula prefix scans preserve tuple literal maplet colons",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper formula scans require scoped binding prefixes",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified bound identifier scans preserve escaped string literal domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper bound-domain checks preserve escaped string literal domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper bound-domain checks include comma-shared bindings",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper bound-domain checks skip tuple-pattern component domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper singleton-domain checks preserve tuple literal elements",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper vacuity checks include unbounded static bodies",
        f"Sumeragi formal README {readme} is missing required text: "
        "Line comment scans preserve escaped string literal comment markers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static outer wrapper scans preserve escaped string literal parentheses",
        f"Sumeragi formal README {readme} is missing required text: "
        "Semantic identifier scans ignore escaped string literal contents",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level relation and boolean scans preserve tuple literal operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level relation scans reject whole-body control/action wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level boolean scans preserve escaped string literal operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level boolean/equality detector helpers preserve tuple literal operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level keyword scans preserve tuple literal keywords",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level CASE branch scans preserve tuple literal arms and conditions",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level keyword and CASE branch scans preserve escaped string literal delimiters",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level CASE branch scans distinguish unary temporal boxes from arm separators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas must use their bound identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas must not duplicate bound identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified unused-bound checks include later binding groups",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified unused-bound checks include unbounded bindings",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified bound identifier scans include later tuple-pattern binding groups",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas must not select predicates with control flow",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas must not appear below top-level negation operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified exactness helper formulas are checked through boolean operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Negated quantified helper checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Negated quantified helper checks split top-level boolean operands before peeling negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper body checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper body control-flow checks reject non-transparent `LET` bodies",
        f"Sumeragi formal README {readme} is missing required text: "
        "Existential quantified exactness helper formulas must not weaken exactness chains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide whole-body control-flow predicate-selection helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide nested control-flow predicate-selection helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Nested control-flow predicate-selection checks unwrap one-line `LET` branch aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Control-flow predicate-selection checks unwrap one-line `LET` control aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Nested control-flow predicate-selection checks include non-branch control operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal exactness helper wrappers must not hide control-flow predicate selection",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal exactness LET-alias helper wrappers must name concrete model predicates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide whole-body raw-predicate boolean-composition helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Raw-predicate exactness boolean-composition helper operands are checked through top-level negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Raw-predicate exactness boolean-composition helper operands are checked through stacked top-level negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Raw-predicate exactness boolean-composition helper operands are checked through unary-temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Raw-predicate exactness boolean-composition helper operands are checked through boolean operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide whole-body parameterized-call boolean-composition helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized-call exactness boolean-composition helper operands are checked through top-level negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized-call exactness boolean-composition helper operands are checked through stacked top-level negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized-call exactness boolean-composition helper operands are checked through unary-temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized-call exactness boolean-composition helper operands are checked through boolean operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated parameterized-call exactness boolean-composition helper operands are checked through identity literals",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal exactness helper wrappers must not hide parameterized helper calls",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide whole-body quantified-predicate boolean-composition helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified-predicate exactness boolean-composition helper operands are checked through top-level negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified-predicate exactness boolean-composition helper operands are checked through stacked top-level negation",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified-predicate exactness boolean-composition helper operands are checked through unary-temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified-predicate exactness boolean-composition helper operands are checked through boolean operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated quantified-predicate exactness boolean-composition helper operands are checked through identity literals",
        f"Sumeragi formal README {readme} is missing required text: "
        "Exactness boolean-composition checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal exactness helper wrappers must not hide quantified formulas",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static action/set/choice exactness helper wrappers must not hide quantified formulas",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static action/set/choice exactness helper wrappers traverse structured operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Structured exactness helper operands must not hide quantified formulas",
        f"Sumeragi formal README {readme} is missing required text: "
        "Structured exactness helper operands must not hide control-flow predicate selection",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal quantified, parameterized-call, and control-flow checks split top-level boolean operands before peeling temporal wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal quantified checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal parameterized-call checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide literal or alias helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide single-helper conjunct aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide self-equality helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must not hide self-inequality helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal self-equality exactness helper wrappers count as self-equality helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal self-inequality exactness helper wrappers count as self-inequality helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Constant-relation exactness helpers count as literal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Constant-relation helper checks unwrap one-line `LET`, unary-temporal, and negated wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static and unary-temporal boolean-only exactness helper wrappers count as",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static IF literal exactness helpers count as literal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static temporal literal checks split top-level boolean operands before peeling temporal or negated wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Negated unary-temporal boolean-only helper wrappers count as literal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound boolean-only temporal helper wrappers count as literal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound exactness helper traversal includes disjunction, implication, equivalence, and negation operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Helper reference traversal unwraps one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Exactness vacuous-helper checks inspect static and structured operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET helper alias unwrapping preserves static unary result wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET binding scans preserve tuple literal definition bodies",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET binding scans preserve escaped string literal definition bodies",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET helper alias unwrapping resolves chained one-line bindings",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET alias substitution respects later quantified binding groups",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET alias substitution respects escaped string literal domain binding groups",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET alias substitution preserves escaped string literal result bodies",
        f"Sumeragi formal README {readme} is missing required text: "
        "LET helper alias unwrapping substitutes simple chained binding references",
        f"Sumeragi formal README {readme} is missing required text: "
        "Temporal literal checks unwrap one-line `LET` helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Non-named correctness-envelope conjuncts are rejected even when mixed",
        f"Sumeragi formal README {readme} is missing required text: "
        "Allowlisted temporal correctness-envelope conjuncts must be non-literal",
        f"Sumeragi formal README {readme} is missing required text: "
        "Allowlisted temporal correctness-envelope conjuncts must be non-self-equality",
        f"Sumeragi formal README {readme} is missing required text: "
        "Allowlisted temporal correctness-envelope conjuncts must be non-self-inequality",
        f"Sumeragi formal README {readme} is missing required text: "
        "Whole-body control-flow temporal side conjuncts must name",
        f"Sumeragi formal README {readme} is missing required text: "
        "Whole-body boolean-composition temporal side conjuncts must name",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal boolean composition over temporal helpers must name",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary `[]`/`<>` boolean-only temporal wrappers count as literal temporal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static IF literal temporal helpers count as literal temporal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal correctness-envelope conjunct chains must not",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide undefined helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas must not hide undefined helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve quantified binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve unbounded quantified binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans reject relation-bearing quantified binding prefixes",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve tuple-pattern quantifier domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve LET binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve parameterized LET operator scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve CHOOSE binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve LAMBDA binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans reject relation-bearing CHOOSE/LAMBDA binding prefixes",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve standard TLA set/operator identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve ENABLED/UNCHANGED operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve CASE branch scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve relation operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve operator-call argument scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve arithmetic/set infix operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve sequence/function infix operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve explicit set literal element scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve unary set-operator operand scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve set-comprehension binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve set-comprehension outer enclosure scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve function-constructor binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve function-set domain and range scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Function-set scans preserve CASE domain branch arrows",
        f"Sumeragi formal README {readme} is missing required text: "
        "Function-set scans preserve record maplet CASE values",
        f"Sumeragi formal README {readme} is missing required text: "
        "Function-set scans preserve record set/update CASE values",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record set field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record update field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve record selector field label scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve comma-shared set/function binding scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Undefined helper scans preserve operator parameter scope",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas must not be vacuous",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper formulas must not restate empty-domain, singleton-domain, bound-domain, self-membership, or empty-set membership facts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper restatement checks reject pure top-level boolean compositions",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper restatement checks reject identity-literal gates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper restatement checks propagate known truth values",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified formula prefix scans preserve escaped string literal colons",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified formula prefix scans preserve tuple literal maplet colons",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper formula scans require scoped binding prefixes",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified bound identifier scans preserve escaped string literal domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper bound-domain checks preserve escaped string literal domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper bound-domain checks include comma-shared bindings",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper bound-domain checks skip tuple-pattern component domains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper singleton-domain checks preserve tuple literal elements",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified helper vacuity checks include unbounded static bodies",
        f"Sumeragi formal README {readme} is missing required text: "
        "Line comment scans preserve escaped string literal comment markers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static outer wrapper scans preserve escaped string literal parentheses",
        f"Sumeragi formal README {readme} is missing required text: "
        "Semantic identifier scans ignore escaped string literal contents",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level relation and boolean scans preserve tuple literal operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level relation scans reject whole-body control/action wrappers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level boolean/equality detector helpers preserve tuple literal operators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level keyword scans preserve tuple literal keywords",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level CASE branch scans preserve tuple literal arms and conditions",
        f"Sumeragi formal README {readme} is missing required text: "
        "Top-level CASE branch scans distinguish unary temporal boxes from arm separators",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas must use their bound identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas must not duplicate bound identifiers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified unused-bound checks include later binding groups",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified unused-bound checks include unbounded bindings",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified bound identifier scans include later tuple-pattern binding groups",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas must not select predicates with control flow",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas must not appear below top-level negation operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Quantified temporal helper formulas are checked through boolean operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Existential quantified temporal helper formulas must not weaken allowlisted temporal chains",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound temporal helper operands must not hide undefined helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide repeated helper conjuncts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Allowlisted temporal helper conjunct repetition checks use the same unary-temporal traversal",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide repeated helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide contradictory helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide excluded-middle helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide complementary-equivalence helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal temporal helper wrappers must not hide repeated helper conjuncts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal temporal helper wrappers must not hide single-helper conjunct aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated temporal helper wrappers must not hide single-helper conjunct aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated temporal helper wrappers must not hide zero-arity helper aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Literal-gated temporal helper wrappers must not hide negated helper operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound temporal helper operands must not hide repeated helper conjuncts",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide whole-body control-flow predicate-selection helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide nested control-flow predicate-selection helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal temporal helper wrappers must not hide control-flow predicate selection",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static action/set/choice temporal helper wrappers must not hide quantified formulas",
        f"Sumeragi formal README {readme} is missing required text: "
        "Static action/set/choice temporal helper wrappers traverse structured operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Structured temporal helper operands must not hide quantified formulas",
        f"Sumeragi formal README {readme} is missing required text: "
        "Structured temporal helper operands must not hide control-flow predicate selection",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal temporal LET-alias helper wrappers must name concrete temporal predicates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide whole-body temporal-helper boolean-composition helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Temporal-helper boolean-composition checks traverse boolean operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Unary-temporal LET-alias temporal side conjuncts must name concrete temporal predicates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide literal or alias helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide single-helper conjunct aliases",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide self-equality helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive allowlisted temporal helper chains must not hide self-inequality helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Constant-relation temporal helpers count as literal temporal helpers",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound `[]`/`<>` temporal helper bodies are traversed for helper references",
        f"Sumeragi formal README {readme} is missing required text: "
        "Parameterized temporal helper calls must be lifted behind zero-arity predicates",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound temporal helper traversal includes disjunction operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound temporal helper traversal includes implication operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound temporal helper traversal includes equivalence operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Compound temporal helper traversal includes negation operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Temporal vacuous-helper checks inspect static and structured operands",
        f"Sumeragi formal README {readme} is missing required text: "
        "Exactness and correctness-envelope conjunct references must resolve to zero-arity",
        f"Sumeragi formal README {readme} is missing required text: "
        "Transitive exactness predicate chains must also resolve through zero-arity",
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


def test_matching_case_prefers_longest_cached_wildcard_prefix() -> None:
    module = load_coverage_module()
    cases = {
        "block-sync-bug-*": module.RunnerCase("block-sync-bug-*", "", 1),
        "block-sync-bug-known-roster-*": module.RunnerCase(
            "block-sync-bug-known-roster-*", "", 2
        ),
    }

    case = module.matching_case("block-sync-bug-known-roster-missing", cases)

    assert case is not None
    assert case.label == "block-sync-bug-known-roster-*"


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


def test_expected_failure_marker_check_ignores_comments_or_malformed_assignment(
) -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            "\n".join(
                [
                    "",
                    "    # expect_failure=1",
                    "    expect_failure = 1",
                ]
            ),
            10,
        )
    }

    assert module.modes_without_expected_failure_marker(
        {"frontier-bug-stale-owner"},
        cases,
        "Apalache",
    ) == [
        "frontier-bug-stale-owner: Apalache runner case 'frontier-bug-*' "
        "at line 10"
    ]


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


def test_expected_failure_default_errors_accepts_top_level_zero(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "sumeragi_apalache.sh"
    script.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "expect_failure=0",
                'case "$mode" in',
                "  frontier-bug-*)",
                "    expect_failure=1",
                "    ;;",
                "esac",
                'if [[ "$expect_failure" == "1" ]]; then',
                "  echo expected",
                "fi",
            ]
        ),
        encoding="utf-8",
    )

    assert module.expected_failure_default_errors(script, "Apalache") == []


def test_expected_failure_default_errors_rejects_global_downgrade(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "sumeragi_apalache.sh"
    script.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "expect_failure=1",
                "expect_failure = 0",
                'case "$mode" in',
                "  frontier-bug-*)",
                "    expect_failure=1",
                "    ;;",
                "esac",
            ]
        ),
        encoding="utf-8",
    )

    assert module.expected_failure_default_errors(script, "Apalache") == [
        f"Apalache runner {script}:3 has malformed top-level "
        "expect_failure assignment: expect_failure = 0",
        f"Apalache runner {script}:2 must set top-level expect_failure "
        "default to 0",
    ]


def test_expected_failure_assignment_errors_rejects_malformed_case_assignment(
) -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            "\n".join(
                [
                    "",
                    "    expect_failure = 1",
                    "    expect_failure=0",
                ]
            ),
            10,
        )
    }

    assert module.expected_failure_assignment_errors(
        {"frontier-bug-stale-owner"},
        cases,
        "Apalache",
    ) == [
        "frontier-bug-stale-owner: Apalache runner case 'frontier-bug-*' "
        "line 11 has malformed expect_failure assignment: expect_failure = 1",
        "frontier-bug-stale-owner: Apalache runner case 'frontier-bug-*' "
        "at line 10 sets expect_failure=0 inside a mode case; keep the "
        "default at top level",
    ]


def test_expected_failure_assignment_errors_rejects_duplicate_case_assignment(
) -> None:
    module = load_coverage_module()
    cases = {
        "frontier-bug-*": module.RunnerCase(
            "frontier-bug-*",
            "\n".join(
                [
                    "",
                    "    expect_failure=1",
                    "    expect_failure=1",
                ]
            ),
            10,
        )
    }

    assert module.expected_failure_assignment_errors(
        {"frontier-bug-stale-owner"},
        cases,
        "TLC",
    ) == [
        "frontier-bug-stale-owner: TLC runner case 'frontier-bug-*' "
        "at line 10 assigns expect_failure 2 times"
    ]


def test_apalache_typecheck_only_mode_errors_accepts_fast_smoke() -> None:
    module = load_coverage_module()
    cases = {
        "fast": module.RunnerCase("fast", "\n    typecheck_only=1\n", 10),
        "frontier-fast": module.RunnerCase("frontier-fast", "\n", 20),
    }

    assert (
        module.apalache_typecheck_only_mode_errors(
            {"fast", "frontier-fast"},
            cases,
        )
        == []
    )


def test_apalache_typecheck_default_errors_accepts_top_level_zero(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "sumeragi_apalache.sh"
    script.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "typecheck_only=0",
                'case "$mode" in',
                "  fast)",
                "    typecheck_only=1",
                "    ;;",
                "esac",
                'if [[ "$typecheck_only" == "1" ]]; then',
                "  apalache-mc typecheck",
                "fi",
            ]
        ),
        encoding="utf-8",
    )

    assert module.apalache_typecheck_default_errors(script) == []


def test_apalache_typecheck_default_errors_rejects_global_downgrade(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    script = tmp_path / "sumeragi_apalache.sh"
    script.write_text(
        "\n".join(
            [
                "#!/bin/bash",
                "typecheck_only=1",
                "typecheck_only = 0",
                'case "$mode" in',
                "  fast)",
                "    typecheck_only=1",
                "    ;;",
                "esac",
            ]
        ),
        encoding="utf-8",
    )

    assert module.apalache_typecheck_default_errors(script) == [
        f"Apalache runner {script}:3 has malformed top-level "
        "typecheck_only assignment: typecheck_only = 0",
        f"Apalache runner {script}:2 must set top-level typecheck_only "
        "default to 0",
    ]


def test_apalache_typecheck_only_mode_errors_rejects_unlisted_modes() -> None:
    module = load_coverage_module()
    cases = {
        "fast": module.RunnerCase("fast", "\n    typecheck_only=1\n", 10),
        "frontier-fast": module.RunnerCase(
            "frontier-fast", "\n    typecheck_only=1\n", 20
        ),
    }

    assert module.apalache_typecheck_only_mode_errors(
        {"fast", "frontier-fast"},
        cases,
    ) == [
        "frontier-fast: Apalache runner case 'frontier-fast' at line 20 "
        "sets typecheck_only=1 outside APALACHE_TYPECHECK_ONLY_MODES"
    ]


def test_apalache_typecheck_only_mode_errors_rejects_malformed_case_assignment(
) -> None:
    module = load_coverage_module()
    cases = {
        "fast": module.RunnerCase("fast", "\n    typecheck_only=1\n", 10),
        "frontier-fast": module.RunnerCase(
            "frontier-fast",
            "\n".join(
                [
                    "",
                    "    typecheck_only = 1",
                    "    typecheck_only=0",
                ]
            ),
            20,
        ),
    }

    assert module.apalache_typecheck_only_mode_errors(
        {"fast", "frontier-fast"},
        cases,
    ) == [
        "frontier-fast: Apalache runner case 'frontier-fast' line 21 "
        "has malformed typecheck_only assignment: typecheck_only = 1",
        "frontier-fast: Apalache runner case 'frontier-fast' at line 20 "
        "sets typecheck_only=0 inside a mode case; keep the default at top level",
    ]


def test_apalache_typecheck_only_mode_errors_rejects_stale_allowlist() -> None:
    module = load_coverage_module()
    cases = {
        "fast": module.RunnerCase("fast", "\n", 10),
        "frontier-fast": module.RunnerCase("frontier-fast", "\n", 20),
    }

    assert module.apalache_typecheck_only_mode_errors(
        {"fast", "frontier-fast"},
        cases,
    ) == [
        "fast: listed in APALACHE_TYPECHECK_ONLY_MODES but Apalache runner "
        "case 'fast' at line 10 does not set typecheck_only=1"
    ]


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
    separator_end = tmp_path / "SumeragiSeparatorEnd.tla"
    malformed_separator = tmp_path / "SumeragiMalformedSeparator.tla"
    malformed_end = tmp_path / "SumeragiMalformedEnd.tla"
    invalid_identifier = tmp_path / "SumeragiInvalidIdentifier.tla"
    malformed_header = tmp_path / "SumeragiMalformedHeader.tla"
    no_separator_header = tmp_path / "SumeragiNoSeparatorHeader.tla"
    reserved_header = tmp_path / "TRUE.tla"
    indented_header = tmp_path / "SumeragiIndentedHeader.tla"
    indented_end = tmp_path / "SumeragiIndentedEnd.tla"
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
    separator_end.write_text(
        "---- MODULE SumeragiSeparatorEnd ----\n"
        "=============================================================================\n"
        "====\n",
        encoding="utf-8",
    )
    malformed_separator.write_text(
        "---- MODULE SumeragiMalformedSeparator ----\n"
        "===== extra\n"
        "====\n",
        encoding="utf-8",
    )
    malformed_end.write_text(
        "---- MODULE SumeragiMalformedEnd ----\n==== extra\n",
        encoding="utf-8",
    )
    invalid_identifier.write_text("---- MODULE 123Bad ----\n====\n", encoding="utf-8")
    malformed_header.write_text(
        "---- MODULE SumeragiMalformedHeader ---- extra\n====\n",
        encoding="utf-8",
    )
    no_separator_header.write_text(
        "---- MODULESumeragiNoSeparatorHeader ----\n====\n",
        encoding="utf-8",
    )
    reserved_header.write_text("---- MODULE TRUE ----\n====\n", encoding="utf-8")
    indented_header.write_text(
        "  ---- MODULE SumeragiIndentedHeader ----\n====\n",
        encoding="utf-8",
    )
    indented_end.write_text(
        "---- MODULE SumeragiIndentedEnd ----\n  ====\n",
        encoding="utf-8",
    )

    assert module.tla_module_header_errors("frontier-fast", [matching, separator_end]) == []
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
            malformed_end,
            malformed_separator,
            invalid_identifier,
            malformed_header,
            no_separator_header,
            reserved_header,
            indented_header,
            indented_end,
        ],
    ) == [
        f"frontier-fast: {mismatched} declares MODULE Different, expected SumeragiQuorum",
        f"frontier-fast: {missing} has no TLA MODULE declaration",
        f"frontier-fast: {duplicate} declares TLA MODULE 2 times",
        f"frontier-fast: {prefixed}:2 declares MODULE after content at line 1",
        f"frontier-fast: {no_end} declares TLA terminator 0 times",
        f"frontier-fast: {duplicate_end} declares TLA terminator 2 times",
        f"frontier-fast: {trailing_end}:2 has content after TLA terminator",
        f"frontier-fast: {malformed_end}:2 malformed TLA terminator: ==== extra",
        f"frontier-fast: {malformed_end} declares TLA terminator 0 times",
        f"frontier-fast: {malformed_separator}:2 malformed TLA terminator: ===== extra",
        f"frontier-fast: {invalid_identifier}:1 malformed TLA MODULE "
        "declaration: ---- MODULE 123Bad ----",
        f"frontier-fast: {invalid_identifier} has no TLA MODULE declaration",
        f"frontier-fast: {malformed_header}:1 malformed TLA MODULE "
        "declaration: ---- MODULE SumeragiMalformedHeader ---- extra",
        f"frontier-fast: {malformed_header} has no TLA MODULE declaration",
        f"frontier-fast: {no_separator_header}:1 malformed TLA MODULE "
        "declaration: ---- MODULESumeragiNoSeparatorHeader ----",
        f"frontier-fast: {no_separator_header} has no TLA MODULE declaration",
        f"frontier-fast: {reserved_header}:1 declares reserved TLA MODULE name TRUE",
        f"frontier-fast: {indented_header}:1 TLA MODULE declaration must be top-level",
        f"frontier-fast: {indented_end}:2 TLA terminator must be top-level",
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
                "INIT=Init",
                "CHECK_DEADLOCK=FALSE",
                "  CHECK_DEADLOCK_FALSE",
                "INVARANT TypeInvariant",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_directive_errors(cfg) == [
        f"{cfg}:5 CHECK_DEADLOCK must be TRUE or FALSE",
        f"{cfg}:6 malformed CFG directive INIT: INIT=Init",
        f"{cfg}:7 malformed CFG directive CHECK_DEADLOCK: CHECK_DEADLOCK=FALSE",
        f"{cfg}:8 indented CFG directive CHECK_DEADLOCK must be top-level",
        f"{cfg}:9 unknown CFG directive INVARANT",
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


def test_cfg_directive_errors_allow_directive_prefixed_block_entries(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "DirectivePrefixedNames.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTANTS",
                "  CHECK_DEADLOCK_FALSE = FALSE",
                "  INIT_MODE = init",
                "INVARIANTS",
                "  INVARIANT_HELPER",
                "  PROPERTY_HOLDS",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_directive_errors(cfg) == []


def test_cfg_directive_errors_rejects_indented_no_separator_directive_starts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "IndentedNoSeparatorDirectives.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "CONSTANTS",
                "  CHECK_DEADLOCK_FALSE = FALSE",
                "  INIT_MODE = init",
                "  INITMode = init",
                "  CHECK_DEADLOCKFalse = FALSE",
                "INVARIANTS",
                "  INVARIANT_HELPER",
                "  PROPERTY_HOLDS",
                "  INVARIANTHelper",
                "  PROPERTIESReady",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_directive_errors(cfg) == [
        f"{cfg}:6 indented CFG directive INIT must be top-level",
        f"{cfg}:7 indented CFG directive CHECK_DEADLOCK must be top-level",
        f"{cfg}:11 indented CFG directive INVARIANT must be top-level",
        f"{cfg}:12 indented CFG directive PROPERTIES must be top-level",
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


def test_cfg_check_operator_names_returns_only_invariants_and_properties(
    tmp_path: Path,
) -> None:
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
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_check_operator_names(cfg) == (
        {"TypeInvariant", "SafetyFast", "BugCheck", "EventuallyCommits"},
        [],
    )
    assert module.cfg_check_operator_kinds(cfg) == (
        {
            "TypeInvariant": "INVARIANT",
            "SafetyFast": "INVARIANT",
            "BugCheck": "INVARIANT",
            "EventuallyCommits": "PROPERTY",
        },
        [],
    )


def test_top_level_cfg_check_parity_errors_accepts_matching_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    deep_cfg = tmp_path / "Sumeragi_deep.cfg"
    tlc_cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    deep_cfg.write_text(
        "\n".join(
            [
                "SPECIFICATION Spec",
                "INVARIANT TypeInvariant",
                "PROPERTIES",
                "  EventuallyCommit",
                "  CommitNeverRevoked",
            ]
        ),
        encoding="utf-8",
    )
    tlc_cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTIES",
                "  EventuallyCommit",
                "  CommitNeverRevoked",
            ]
        ),
        encoding="utf-8",
    )

    assert module.top_level_cfg_check_parity_errors(deep_cfg, tlc_cfg) == []


def test_top_level_cfg_check_parity_errors_rejects_divergent_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    deep_cfg = tmp_path / "Sumeragi_deep.cfg"
    tlc_cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    deep_cfg.write_text(
        "\n".join(
            [
                "SPECIFICATION Spec",
                "INVARIANT TypeInvariant",
                "PROPERTY ApalacheOnlyCheck",
            ]
        ),
        encoding="utf-8",
    )
    tlc_cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY TlcOnlyCheck",
            ]
        ),
        encoding="utf-8",
    )

    assert module.top_level_cfg_check_parity_errors(deep_cfg, tlc_cfg) == [
        f"{tlc_cfg} is missing top-level check ApalacheOnlyCheck from {deep_cfg}",
        f"{deep_cfg} is missing top-level check TlcOnlyCheck from {tlc_cfg}",
    ]


def test_top_level_cfg_check_parity_errors_rejects_check_kind_drift(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    deep_cfg = tmp_path / "Sumeragi_deep.cfg"
    tlc_cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    deep_cfg.write_text(
        "\n".join(
            [
                "SPECIFICATION Spec",
                "INVARIANT TypeInvariant",
                "PROPERTY EventuallyCommit",
            ]
        ),
        encoding="utf-8",
    )
    tlc_cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANTS TypeInvariant EventuallyCommit",
            ]
        ),
        encoding="utf-8",
    )

    assert module.top_level_cfg_check_parity_errors(deep_cfg, tlc_cfg) == [
        f"top-level check EventuallyCommit is PROPERTY in {deep_cfg} "
        f"but INVARIANT in {tlc_cfg}"
    ]


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
                "INVARIANT, TypeInvariant",
                "INVARIANTS Safety Good",
                "PROPERTIES, EventuallyCommits",
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
            (5, "INVARIANTS", "Safety"),
            (5, "INVARIANTS", "Good"),
            (7, "INVARIANTS", "Safety"),
            (11, "PROPERTIES", "EventuallyRecovers"),
        ],
        [
            f"{cfg}:1 directive SPECIFICATION must reference a static TLA operator: Spec!",
            f"{cfg}:2 directive NEXT must reference a static TLA operator: THEOREM",
            f"{cfg}:3 directive PROPERTY must reference a static TLA operator: WF_vars",
            f"{cfg}:4 directive INVARIANT must reference exactly one operator: "
            "INVARIANT, TypeInvariant",
            f"{cfg}:6 directive PROPERTIES must reference static TLA operators: "
            "PROPERTIES, EventuallyCommits",
            f"{cfg}:7 directive INVARIANTS must reference static TLA operators: Bad-Name",
            f"{cfg}:7 directive INVARIANTS must reference static TLA operators: TRUE",
            f"{cfg}:9 PROPERTIES block line must reference exactly one static TLA operator",
            f"{cfg}:10 PROPERTIES block line must reference exactly one static TLA operator",
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


def test_cfg_operator_references_reject_no_separator_directive_starts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "NoSeparatorOperators.cfg"
    cfg.write_text(
        "\n".join(
            [
                "SPECIFICATIONSpec",
                "INVARIANTTypeInvariant",
                "PROPERTIESEventuallyCommits",
                "INVARIANTS",
                "  INVARIANT_HELPER",
                "PROPERTIES",
                "  PROPERTY_READY",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [
            (5, "INVARIANTS", "INVARIANT_HELPER"),
            (7, "PROPERTIES", "PROPERTY_READY"),
        ],
        [
            f"{cfg}:1 directive SPECIFICATION must reference exactly one "
            "operator: SPECIFICATIONSpec",
            f"{cfg}:2 directive INVARIANT must reference exactly one "
            "operator: INVARIANTTypeInvariant",
            f"{cfg}:3 directive PROPERTIES must reference static TLA "
            "operators: PROPERTIESEventuallyCommits",
        ],
    )


def test_cfg_operator_references_reject_indented_no_separator_directive_starts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "IndentedNoSeparatorOperators.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INVARIANTS",
                "  INVARIANT_HELPER",
                "  INVARIANTTypeInvariant",
                "INVARIANTS",
                "  NEXT_STEP",
                "  NEXTStep",
                "PROPERTIES",
                "  PROPERTY_READY",
                "  PROPERTYEventuallyCommits",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_operator_references(cfg) == (
        [
            (2, "INVARIANTS", "INVARIANT_HELPER"),
            (5, "INVARIANTS", "NEXT_STEP"),
            (8, "PROPERTIES", "PROPERTY_READY"),
        ],
        [
            f"{cfg}:3 indented CFG directive INVARIANT must be top-level",
            f"{cfg}:6 indented CFG directive NEXT must be top-level",
            f"{cfg}:9 indented CFG directive PROPERTY must be top-level",
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
                "  INVARIANT, TypeInvariant",
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
            f"{cfg}:3 indented CFG directive INVARIANT must be top-level",
            f"{cfg}:5 indented CFG directive CHECK_DEADLOCK must be top-level",
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


def test_tla_operator_definitions_parse_plain_and_parameterized(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Parameterized(value) ==",
                "  ScopedLetHelper ==",
                "TypeInvariant ==",
                "RECURSIVEFoo == TRUE",
                "INSTANCEImported == TRUE",
                "varsTuple == TRUE",
                "RECURSIVE RecursiveOne(_), RecursiveTwo(_, _)",
                "RecursiveOne(value) == TRUE",
                "RecursiveTwo(left, right) == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_operator_definitions(tla) == {
        "Parameterized",
        "INSTANCEImported",
        "RECURSIVEFoo",
        "RecursiveOne",
        "RecursiveTwo",
        "TypeInvariant",
        "varsTuple",
    }


def test_tla_duplicate_operator_definition_errors_rejects_local_definitions(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "LOCAL Helper == TRUE",
                "LOCAL\tTabbed == TRUE",
                "RealHelper == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:2 TLA operator definition must be "
        "non-LOCAL: LOCAL Helper == TRUE",
        f"frontier-fast: {tla}:3 TLA operator definition must be "
        "non-LOCAL: LOCAL\tTabbed == TRUE",
    ]
    assert module.tla_operator_definitions(tla) == {"RealHelper"}


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


def test_tla_duplicate_operator_definition_errors_rejects_duplicate_parameters(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Duplicate(value, value) == TRUE",
                "RECURSIVE RecursiveDuplicate(left, left), Placeholder(_, _)",
                "RecursiveDuplicate(left, right) == TRUE",
                "Placeholder(left, right) == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_duplicate_operator_definition_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:3 RECURSIVE declaration must use unique static "
        "operator parameters: RecursiveDuplicate(left, left)",
        f"frontier-fast: {tla}:2 TLA operator definition must use unique "
        "static parameters: Duplicate(value, value) == TRUE",
    ]
    assert module.tla_operator_definitions(tla) == {
        "Placeholder",
        "RecursiveDuplicate",
    }


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
                "Multiline ==",
                "  TRUE",
                "Wrapped ==",
                "  /\\ TRUE",
                "ConjLiteral == TRUE /\\ TRUE",
                "UsesLet ==",
                "  LET scoped == FALSE",
                "  IN scoped",
                "RealCheck == TRUE /\\ Safety",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_literal_operator_definitions(tla) == {
        "LiteralTrue": (2, "TRUE"),
        "LiteralFalse": (3, "FALSE"),
        "Multiline": (5, "TRUE"),
        "Wrapped": (7, "/\\ TRUE"),
        "ConjLiteral": (8, "TRUE /\\ TRUE"),
    }


def test_tla_line_comments_preserve_escaped_string_comment_markers(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()

    assert (
        module.tla_line_without_comment(
            'Value == "left \\* not comment" \\* real comment'
        )
        == 'Value == "left \\* not comment" '
    )
    assert (
        module.tla_line_without_comment(
            'Value == "left \\" \\* still string" \\* real comment'
        )
        == 'Value == "left \\" \\* still string" '
    )

    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                'StringPredicate == checked = "ready \\* text" \\* real comment',
                "MultilineString ==",
                '  /\\ checked = "line \\* still literal" \\* real comment',
                "  /\\ ConcretePredicate",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert definitions["StringPredicate"] == (
        2,
        'checked = "ready \\* text"',
    )
    assert definitions["MultilineString"] == (
        4,
        '/\\ checked = "line \\* still literal" /\\ ConcretePredicate',
    )


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
                "WrappedAlias == (TypeInvariant)",
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
        "WrappedAlias": (5, "(TypeInvariant)"),
    }


def test_tla_static_boolean_literal_evaluates_obvious_wrappers() -> None:
    module = load_coverage_module()

    assert module.tla_static_boolean_literal("TRUE /\\ TRUE") == "TRUE"
    assert module.tla_static_boolean_literal("(FALSE \\/ FALSE)") == "FALSE"
    assert module.tla_static_boolean_literal("~FALSE") == "TRUE"
    assert module.tla_static_boolean_literal("/\\ TRUE") == "TRUE"
    assert module.tla_static_boolean_literal("counter = 0") is None
    assert module.tla_static_boolean_literal("TRUE /\\ Safety") is None


def test_tla_static_temporal_boolean_literal_evaluates_unary_wrappers() -> None:
    module = load_coverage_module()

    assert module.tla_static_temporal_boolean_literal("[] TRUE") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("<> FALSE") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("[] (TRUE /\\ TRUE)") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("<> ([] FALSE)") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("[] (TRUE /\\ Safety)") is None
    assert module.tla_static_temporal_boolean_literal("~([] FALSE)") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("~(<> TRUE)") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("~[] FALSE /\\ FALSE") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("~([] FALSE) /\\ FALSE") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("~([] FALSE /\\ FALSE)") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("~[] TRUE \\/ TRUE") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("~([] TRUE) \\/ TRUE") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("~([] TRUE \\/ TRUE)") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("[] (TRUE => TRUE)") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("[] (TRUE <=> FALSE)") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("[] ([] TRUE /\\ <> TRUE)") == "TRUE"
    assert module.tla_static_temporal_boolean_literal("[] ([] FALSE \\/ <> FALSE)") == "FALSE"
    assert module.tla_static_temporal_boolean_literal("[] ([] TRUE /\\ Safety)") is None
    assert module.tla_static_temporal_boolean_literal(
        "LET selected == TRUE IN selected"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "[] (LET selected == TRUE IN selected)"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "LET selected == TRUE IN [] selected"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "~(LET selected == FALSE IN selected)"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "LET first == TRUE second == first IN second"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "LET first == TRUE second == first IN [] second"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "~(LET first == FALSE second == first IN second)"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "LET first == TRUE second == first /\\ TRUE IN second"
    ) == "TRUE"
    assert module.tla_static_temporal_boolean_literal(
        "LET first == FALSE second == first \\/ FALSE IN second"
    ) == "FALSE"


def test_tla_static_if_boolean_literal_evaluates_static_condition_branch() -> None:
    module = load_coverage_module()

    assert module.tla_static_if_boolean_literal(
        "IF TRUE THEN TRUE ELSE FALSE"
    ) == "TRUE"
    assert module.tla_static_if_boolean_literal(
        "IF FALSE THEN TRUE ELSE FALSE"
    ) == "FALSE"
    assert module.tla_static_if_boolean_literal(
        "IF TRUE THEN (IF FALSE THEN TRUE ELSE FALSE) ELSE TRUE"
    ) == "FALSE"
    assert module.tla_static_if_boolean_literal(
        "IF [] TRUE THEN <> TRUE ELSE FALSE"
    ) == "TRUE"
    assert (
        module.tla_static_if_boolean_literal(
            "IF ready THEN TRUE ELSE FALSE"
        )
        is None
    )
    assert (
        module.tla_static_if_boolean_literal(
            "IF TRUE THEN ModelPredicate ELSE FALSE"
        )
        is None
    )
    assert (
        module.tla_static_if_boolean_literal(
            "CASE TRUE -> TRUE [] OTHER -> FALSE"
        )
        is None
    )


def test_tla_static_constant_relation_detects_identifier_free_relations() -> None:
    module = load_coverage_module()

    assert module.tla_static_constant_relation("TRUE = TRUE") == "TRUE = TRUE"
    assert module.tla_static_constant_relation("FALSE # TRUE") == "FALSE # TRUE"
    assert module.tla_static_constant_relation("1 \\in {1}") == "1 \\in {1}"
    assert module.tla_static_constant_relation('"a" = "a"') == '"a" = "a"'
    assert module.tla_static_constant_relation("{} = {}") == "{} = {}"
    assert (
        module.tla_static_constant_relation("[] (TRUE = TRUE)")
        == "[] (TRUE = TRUE)"
    )
    assert (
        module.tla_static_constant_relation("<> (1 \\in {1})")
        == "<> (1 \\in {1})"
    )
    assert (
        module.tla_static_constant_relation("~([] (FALSE # TRUE))")
        == "~([] (FALSE # TRUE))"
    )
    assert module.tla_static_constant_relation("checked = ready") is None
    assert module.tla_static_constant_relation('"case" \\in tried') is None
    assert module.tla_static_constant_relation("[] (checked = ready)") is None
    assert module.tla_static_constant_relation("TRUE") is None
    assert module.tla_static_constant_relation("TRUE => FALSE") is None
    assert module.tla_static_constant_relation("TRUE <=> FALSE") is None


def test_vacuous_quantified_helper_formulas_detect_bound_self_membership() -> None:
    module = load_coverage_module()

    assert module.vacuous_quantified_helper_formulas("\\A c: TRUE") == [
        "\\A c: TRUE"
    ]
    assert module.vacuous_quantified_helper_formulas("\\E c: [] FALSE") == [
        "\\E c: [] FALSE"
    ]
    assert module.vacuous_quantified_helper_formulas("\\A c: Predicate(c)") == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in {c}"
    ) == ["\\A c \\in Cases: c \\in {c}"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in {Other, c}"
    ) == ["\\A c \\in Cases: c \\in {Other, c}"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in Cases"
    ) == ["\\A c \\in Cases: c \\in Cases"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c, d \\in Cases: c \\in Cases"
    ) == ["\\A c, d \\in Cases: c \\in Cases"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c, d \\in Cases: d \\in Cases"
    ) == ["\\A c, d \\in Cases: d \\in Cases"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A <<c, d>> \\in Pairs: c \\in Pairs"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A <<c, d>> \\in {1}: c = 1"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: [] (c \\in {c})"
    ) == ["\\A c \\in Cases: [] (c \\in {c})"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: ~(c \\notin {c})"
    ) == ["\\A c \\in Cases: ~(c \\notin {c})"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\notin {}"
    ) == ["\\A c \\in Cases: c \\notin {}"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: [] (c \\notin {})"
    ) == ["\\A c \\in Cases: [] (c \\notin {})"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: ~(c \\in {})"
    ) == ["\\A c \\in Cases: ~(c \\in {})"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in Cases /\\ c \\notin {}"
    ) == ["\\A c \\in Cases: c \\in Cases /\\ c \\notin {}"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in Cases \\/ ~(c \\in {})"
    ) == ["\\A c \\in Cases: c \\in Cases \\/ ~(c \\in {})"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: TRUE /\\ c \\in Cases"
    ) == ["\\A c \\in Cases: TRUE /\\ c \\in Cases"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: FALSE \\/ c \\notin {}"
    ) == ["\\A c \\in Cases: FALSE \\/ c \\notin {}"]
    assert (
        module.quantified_formula_restatement_literal(
            "\\A c \\in Cases: c \\in Cases => FALSE"
        )
        == "FALSE"
    )
    assert (
        module.quantified_formula_restatement_literal(
            "\\A c \\in Cases: c \\notin Cases => FALSE"
        )
        == "TRUE"
    )
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in Cases => FALSE"
    ) == ["\\A c \\in Cases: c \\in Cases => FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\notin Cases => FALSE"
    ) == ["\\A c \\in Cases: c \\notin Cases => FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\notin Cases <=> FALSE"
    ) == ["\\A c \\in Cases: c \\notin Cases <=> FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in {} <=> FALSE"
    ) == ["\\A c \\in Cases: c \\in {} <=> FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {}: c = ready"
    ) == ["\\A c \\in {}: c = ready"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {}: [] (c = ready)"
    ) == ["\\A c \\in {}: [] (c = ready)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1"
    ) == ["\\A c \\in {1}: c = 1"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A t \\in {<<1, 2>>}: t = <<1, 2>>"
    ) == ["\\A t \\in {<<1, 2>>}: t = <<1, 2>>"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A t \\in {<<1, 2>>}: <<1, 2>> = t"
    ) == ["\\A t \\in {<<1, 2>>}: <<1, 2>> = t"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c, d \\in {1}: c = 1"
    ) == ["\\A c, d \\in {1}: c = 1"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: 1 = c"
    ) == ["\\A c \\in {1}: 1 = c"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: [] (c = 1)"
    ) == ["\\A c \\in {1}: [] (c = 1)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: ~(c # 1)"
    ) == ["\\A c \\in {1}: ~(c # 1)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 /\\ 1 = c"
    ) == ["\\A c \\in {1}: c = 1 /\\ 1 = c"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 \\/ ~(c # 1)"
    ) == ["\\A c \\in {1}: c = 1 \\/ ~(c # 1)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: TRUE /\\ c = 1"
    ) == ["\\A c \\in {1}: TRUE /\\ c = 1"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 /\\ TRUE"
    ) == ["\\A c \\in {1}: c = 1 /\\ TRUE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: FALSE \\/ c = 1"
    ) == ["\\A c \\in {1}: FALSE \\/ c = 1"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: TRUE => c = 1"
    ) == ["\\A c \\in {1}: TRUE => c = 1"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 <=> TRUE"
    ) == ["\\A c \\in {1}: c = 1 <=> TRUE"]
    assert (
        module.quantified_formula_restatement_literal(
            "\\A c \\in {1}: c = 1 => FALSE"
        )
        == "FALSE"
    )
    assert (
        module.quantified_formula_restatement_literal(
            "\\A c \\in {1}: c # 1 => FALSE"
        )
        == "TRUE"
    )
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 => FALSE"
    ) == ["\\A c \\in {1}: c = 1 => FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c # 1 => FALSE"
    ) == ["\\A c \\in {1}: c # 1 => FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c # 1 <=> FALSE"
    ) == ["\\A c \\in {1}: c # 1 <=> FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 <=> FALSE"
    ) == ["\\A c \\in {1}: c = 1 <=> FALSE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in Cases /\\ Predicate(c)"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: c = 1 /\\ Predicate(c)"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: TRUE /\\ Predicate(c)"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: FALSE \\/ Predicate(c)"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: Predicate(c) => FALSE"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: Predicate(c) <=> FALSE"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in {1}: Predicate(c)"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: c \\in OtherCases"
    ) == []
    assert module.vacuous_quantified_helper_formulas(
        "\\A c, d \\in Cases: c \\in OtherCases"
    ) == []


def test_temporal_helper_references_descend_into_compound_temporal_bodies() -> None:
    module = load_coverage_module()

    assert module.temporal_helper_references("[] (FirstHelper /\\ SecondHelper)") == [
        "FirstHelper",
        "SecondHelper",
    ]
    assert module.temporal_helper_references("[] (FirstHelper \\/ SecondHelper)") == [
        "FirstHelper",
        "SecondHelper",
    ]
    assert module.temporal_helper_references("<> ([] NestedHelper)") == [
        "NestedHelper"
    ]
    assert module.temporal_helper_references("[] (Gst => <> Committed)") == [
        "Committed",
        "Gst",
    ]
    assert module.temporal_helper_references(
        "[] (GateHelper => <> FollowupHelper)"
    ) == [
        "FollowupHelper",
        "GateHelper",
    ]
    assert module.temporal_helper_references(
        "[] (LeftHelper <=> <> RightHelper)"
    ) == [
        "LeftHelper",
        "RightHelper",
    ]
    assert module.temporal_helper_references("[] (~NegatedHelper)") == [
        "NegatedHelper"
    ]
    assert module.unary_temporal_let_alias_parts(
        "[] (LET selected == TemporalConcrete IN selected)"
    ) == [
        "[] (LET selected == TemporalConcrete IN selected) aliases "
        "TemporalConcrete"
    ]
    assert module.unary_temporal_let_alias_parts(
        "<> (LET selected == IF ready THEN TemporalA ELSE TemporalB IN selected)"
    ) == [
        "<> (LET selected == IF ready THEN TemporalA ELSE TemporalB IN "
        "selected) aliases IF ready THEN TemporalA ELSE TemporalB"
    ]
    assert module.unary_temporal_let_alias_parts("[] TemporalConcrete") == []


def test_hidden_helper_references_descend_into_static_and_structured_operands() -> None:
    module = load_coverage_module()

    assert module.hidden_static_structured_helper_references("ENABLED LiteralLeaf") == [
        "LiteralLeaf"
    ]
    assert module.hidden_static_structured_helper_references("DOMAIN LiteralLeaf") == [
        "LiteralLeaf"
    ]
    assert module.hidden_static_structured_helper_references(
        "CHOOSE BoundHelper \\in Cases: BoundHelper /\\ LiteralLeaf"
    ) == ["Cases", "LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "LAMBDA BoundHelper \\in Cases: BoundHelper /\\ LiteralLeaf"
    ) == ["Cases", "LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "[Field |-> LiteralLeaf] /\\ ConcreteLeaf"
    ) == ["LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "ActualLeaf = LiteralLeaf"
    ) == []
    assert module.hidden_static_structured_helper_references(
        "[Field |-> ActualLeaf = LiteralLeaf]"
    ) == ["ActualLeaf", "LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "[BoundHelper \\in Cases |-> BoundHelper /\\ LiteralLeaf]"
    ) == ["Cases", "LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "Helper(LiteralLeaf)"
    ) == ["LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "[] [Field |-> LiteralLeaf]"
    ) == ["LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "[] (ENABLED LiteralLeaf /\\ OtherLeaf)"
    ) == ["LiteralLeaf"]
    assert module.hidden_static_structured_helper_references(
        "IF Ready THEN LiteralLeaf ELSE ConcreteLeaf"
    ) == []


def test_structured_control_flow_formulas_descend_into_structured_operands() -> None:
    module = load_coverage_module()
    definitions = {
        "ConcreteLeaf": (1, "checked = ready"),
        "FallbackLeaf": (2, "checked = committed"),
        "QuorumMarker": (3, "1"),
        "VoteBackedMarker": (4, "2"),
    }

    assert module.structured_operand_control_flow_formulas(
        "IF Ready THEN ConcreteLeaf ELSE FallbackLeaf",
        definitions,
    ) == []
    assert module.structured_operand_control_flow_formulas(
        "[Field |-> IF Ready THEN ConcreteLeaf ELSE FallbackLeaf]",
        definitions,
    ) == [
        "record literal contains IF expression "
        "IF Ready THEN ConcreteLeaf ELSE FallbackLeaf"
    ]
    assert module.structured_operand_control_flow_formulas(
        "[Field |-> CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf]",
        definitions,
    ) == [
        "record literal contains CASE expression "
        "CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf"
    ]
    assert module.structured_operand_control_flow_formulas(
        "[Field: CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf]",
        definitions,
    ) == [
        "record set contains CASE expression "
        "CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf"
    ]
    assert module.structured_operand_control_flow_formulas(
        "[record EXCEPT !.field = CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf]",
        definitions,
    ) == [
        "record update contains CASE expression "
        "CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf"
    ]
    assert module.structured_operand_control_flow_formulas(
        "[CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf -> Cases]",
        definitions,
    ) == [
        "function set contains CASE expression "
        "CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf"
    ]
    assert module.structured_operand_control_flow_formulas(
        "[Field |-> ENABLED ConcreteLeaf]",
        definitions,
    ) == [
        "record literal contains ENABLED expression ENABLED ConcreteLeaf"
    ]
    assert module.structured_operand_control_flow_formulas(
        "ActualMarker = IF Ready THEN VoteBackedMarker ELSE QuorumMarker",
        definitions,
    ) == []


def test_helper_reference_traversal_unwraps_one_line_let_aliases() -> None:
    module = load_coverage_module()

    assert module.tla_zero_arity_boolean_references(
        "ConcretePredicate /\\ (LET selected == HiddenLiteral IN selected)"
    ) == ["ConcretePredicate", "HiddenLiteral"]
    assert module.tla_static_let_alias_operand(
        "LET selected == HiddenLiteral IN [] selected"
    ) == "[] (HiddenLiteral)"
    assert module.tla_static_let_alias_operand(
        "LET selected == HiddenLiteral IN ~<> selected"
    ) == "~ (<> (HiddenLiteral))"
    assert module.tla_static_let_alias_operand(
        "LET first == HiddenLiteral second == first IN [] second"
    ) == "[] (HiddenLiteral)"
    assert module.tla_static_let_alias_operand(
        "LET first == HiddenLiteral second == first /\\ OtherLiteral IN second"
    ) == "(HiddenLiteral) /\\ OtherLiteral"
    assert module.tla_static_let_alias_operand(
        "LET selected == <<left == right, tail>> IN selected"
    ) == "<<left == right, tail>>"
    assert module.tla_static_let_alias_operand(
        'LET selected == "left \\" == right" IN selected'
    ) == '"left \\" == right"'
    assert module.tla_static_let_alias_operand(
        'LET first == TRUE second == "left \\" == right" IN first'
    ) == "TRUE"
    assert module.tla_static_let_alias_operand(
        "LET snapshot == <<left == right, tail>> selected == HiddenLiteral IN [] selected"
    ) == "[] (HiddenLiteral)"
    later_bound_alias = "\\A c \\in Cases, selected \\in Other: Predicate(selected)"
    assert module.tla_quantified_bound_identifiers(later_bound_alias) == {
        "c",
        "selected",
    }
    string_domain_bound_alias = (
        '\\A selected \\in "left \\" : right": Predicate(selected)'
    )
    assert module.tla_quantifier_prefixes(
        '"literal \\\\A hidden \\\\in Cases: Hidden" /\\ ' + later_bound_alias
    ) == [later_bound_alias.rsplit(":", 1)[0]]
    assert module.tla_quantified_bound_identifiers(string_domain_bound_alias) == {
        "selected",
    }
    assert (
        module.tla_static_substitute_let_alias_references(
            later_bound_alias,
            [("selected", "HiddenLiteral")],
        )
        is None
    )
    assert (
        module.tla_static_substitute_let_alias_references(
            string_domain_bound_alias,
            [("selected", "HiddenLiteral")],
        )
        is None
    )
    assert (
        module.tla_static_let_alias_operand(
            "LET selected == HiddenLiteral IN " + later_bound_alias
        )
        is None
    )
    assert (
        module.tla_static_let_alias_operand(
            "LET selected == HiddenLiteral IN " + string_domain_bound_alias
        )
        is None
    )
    assert (
        module.tla_static_let_alias_operand(
            "LET first == second second == first IN first"
        )
        is None
    )
    assert module.tla_static_let_alias_operand(
        "LET first == HiddenLiteral second == first(Case) IN second"
    ) == "first(Case)"
    assert module.tla_static_let_alias_operand(
        "LET first == HiddenLiteral second == \"first\" IN second"
    ) == '"first"'
    assert (
        module.tla_static_substitute_let_alias_references(
            '"selected \\" selected"',
            [("selected", "HiddenLiteral")],
        )
        is None
    )
    assert module.tla_static_let_alias_operand(
        'LET selected == HiddenLiteral wrapped == "selected \\" selected" IN wrapped'
    ) == '"selected \\" selected"'
    assert module.exactness_helper_references(
        "ConcretePredicate /\\ (LET selected == HiddenLiteral IN selected)"
    ) == ["ConcretePredicate", "HiddenLiteral"]
    assert module.exactness_helper_references(
        "ConcretePredicate /\\ (LET selected == HiddenLiteral IN [] selected)"
    ) == ["ConcretePredicate", "HiddenLiteral"]
    assert module.temporal_helper_references(
        "ConcreteTemporal /\\ (LET selected == TemporalHiddenLiteral IN selected)"
    ) == ["ConcreteTemporal", "TemporalHiddenLiteral"]
    assert module.temporal_helper_references(
        "ConcreteTemporal /\\ (LET selected == TemporalHiddenLiteral IN <> selected)"
    ) == ["ConcreteTemporal", "TemporalHiddenLiteral"]
    assert module.quantified_helper_formulas(
        "ConcretePredicate /\\ (LET q == \\A c \\in Cases: Predicate(c) IN q)"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.temporal_direct_boolean_parts(
        "ConcreteTemporal /\\ (LET selected == checked = checked IN selected)"
    ) == [
        "ConcreteTemporal /\\ (LET selected == checked = checked IN selected)",
        "ConcreteTemporal",
        "LET selected == checked = checked IN selected",
        "checked = checked",
    ]
    assert module.temporal_undefined_helper_references(
        "ConcreteTemporal /\\ (LET selected == TemporalMissingPredicate IN selected)"
    ) == ["ConcreteTemporal", "TemporalMissingPredicate"]
    assert module.tla_zero_arity_boolean_references(
        "ConcretePredicate /\\ (LET selected == HiddenLiteral IN other)"
    ) == ["ConcretePredicate"]


def test_zero_arity_polarity_conflicts_descend_into_unary_temporal_operands() -> None:
    module = load_coverage_module()

    assert module.zero_arity_operand_polarity("[] Leaf") == ("Leaf", True)
    assert module.zero_arity_operand_polarity("[] ~Leaf") == ("Leaf", False)
    assert module.zero_arity_operand_polarity("~[] Leaf") == ("Leaf", False)
    assert module.zero_arity_operand_polarity("<> ~~Leaf") == ("Leaf", True)
    assert module.contradictory_zero_arity_conjunct_references(
        "Leaf /\\ [] ~Leaf"
    ) == ["Leaf"]
    assert module.excluded_middle_zero_arity_disjunct_references(
        "Leaf \\/ [] ~Leaf"
    ) == ["Leaf"]
    assert module.complementary_equivalence_zero_arity_references(
        "Leaf <=> [] ~Leaf"
    ) == ["Leaf"]
    assert module.complementary_equivalence_zero_arity_references(
        "Leaf <=> Other <=> ~Leaf"
    ) == ["Leaf"]
    assert module.complementary_equivalence_zero_arity_references(
        "Leaf <=> ~Leaf <=> Other"
    ) == ["Leaf"]


def test_zero_arity_polarity_conflicts_unwrap_one_line_let_aliases() -> None:
    module = load_coverage_module()

    assert module.zero_arity_operand_polarity(
        "LET selected == Leaf IN selected"
    ) == ("Leaf", True)
    assert module.zero_arity_operand_polarity(
        "~(LET selected == Leaf IN selected)"
    ) == ("Leaf", False)
    assert module.zero_arity_operand_polarity(
        "LET selected == ~Leaf IN selected"
    ) == ("Leaf", False)
    assert module.zero_arity_operand_polarity(
        "LET selected == Leaf IN [] selected"
    ) == ("Leaf", True)
    assert module.zero_arity_operand_polarity(
        "LET selected == Leaf IN ~selected"
    ) == ("Leaf", False)
    assert module.zero_arity_operand_polarity(
        "LET selected == Leaf IN other"
    ) is None
    assert module.duplicate_zero_arity_wrapped_conjunct_references(
        "Leaf /\\ (LET selected == Leaf IN selected)"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_wrapped_conjunct_references(
        "LET selected == (Leaf /\\ Leaf) IN selected"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf \\/ (LET selected == Leaf IN selected)"
    ) == ["Leaf"]
    assert module.contradictory_zero_arity_conjunct_references(
        "Leaf /\\ ~(LET selected == Leaf IN selected)"
    ) == ["Leaf"]
    assert module.excluded_middle_zero_arity_disjunct_references(
        "Leaf \\/ ~(LET selected == Leaf IN selected)"
    ) == ["Leaf"]
    assert module.complementary_equivalence_zero_arity_references(
        "Leaf <=> ~(LET selected == Leaf IN selected)"
    ) == ["Leaf"]


def test_single_conjunct_alias_detection_unwraps_static_temporal_wrappers() -> None:
    module = load_coverage_module()
    definitions = {
        "Leaf": (1, "checked = ready"),
        "OtherLeaf": (2, "ready = checked"),
    }

    assert module.single_zero_arity_conjunct_alias("/\\ Leaf", definitions) == "Leaf"
    assert (
        module.single_zero_arity_conjunct_alias("[] (/\\ Leaf)", definitions)
        == "Leaf"
    )
    assert (
        module.single_zero_arity_conjunct_alias("~[] (/\\ Leaf)", definitions)
        == "Leaf"
    )
    assert (
        module.single_zero_arity_conjunct_alias(
            "LET selected == /\\ Leaf IN <> selected",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.single_zero_arity_conjunct_alias(
            "TRUE => (/\\ Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.single_zero_arity_conjunct_alias(
            "FALSE \\/ (/\\ Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.single_zero_arity_conjunct_alias(
            "TRUE /\\ (/\\ Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.single_zero_arity_conjunct_alias(
            "TRUE <=> (/\\ Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert module.single_zero_arity_conjunct_alias("[] Leaf", definitions) is None
    assert (
        module.single_zero_arity_conjunct_alias(
            "(/\\ Leaf) \\/ (/\\ OtherLeaf)",
            definitions,
        )
        is None
    )


def test_literal_gated_zero_arity_helper_alias_detection_unwraps_static_wrappers() -> None:
    module = load_coverage_module()
    definitions = {
        "Leaf": (1, "checked = ready"),
        "OtherLeaf": (2, "ready = checked"),
    }

    assert (
        module.literal_gated_zero_arity_helper_alias("TRUE /\\ Leaf", definitions)
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias("FALSE \\/ Leaf", definitions)
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias("TRUE => Leaf", definitions)
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE => TRUE => Leaf",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE => (TRUE => Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias("TRUE <=> Leaf", definitions)
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE <=> TRUE <=> Leaf",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE <=> (TRUE <=> Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "[] (TRUE /\\ Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "LET selected == Leaf IN TRUE /\\ selected",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE /\\ [] Leaf",
            definitions,
        )
        is None
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE /\\ ~Leaf",
            definitions,
        )
        is None
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE => FALSE => Leaf",
            definitions,
        )
        is None
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "TRUE <=> FALSE <=> Leaf",
            definitions,
        )
        is None
    )
    assert (
        module.literal_gated_zero_arity_helper_alias(
            "Leaf \\/ OtherLeaf",
            definitions,
        )
        is None
    )


def test_literal_gated_negated_helper_operand_detection_recurses_identity_gates() -> None:
    module = load_coverage_module()
    definitions = {"Leaf": (1, "checked = ready")}

    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE /\\ ~Leaf",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE /\\ (TRUE /\\ ~Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE => TRUE => ~Leaf",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE => (TRUE => ~Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE <=> TRUE <=> ~Leaf",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE <=> (TRUE <=> ~Leaf)",
            definitions,
        )
        == "Leaf"
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE => FALSE => ~Leaf",
            definitions,
        )
        is None
    )
    assert (
        module.literal_gated_negated_zero_arity_helper_operand(
            "TRUE <=> FALSE <=> ~Leaf",
            definitions,
        )
        is None
    )


def test_repeated_zero_arity_conjuncts_descend_into_unary_temporal_operands() -> None:
    module = load_coverage_module()

    assert module.duplicate_zero_arity_wrapped_conjunct_references(
        "Leaf /\\ [] Leaf"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_wrapped_conjunct_references(
        "[] (Leaf /\\ <> Leaf)"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_wrapped_conjunct_references(
        "Leaf /\\ ~[] ~Leaf"
    ) == ["Leaf"]
    assert (
        module.duplicate_zero_arity_wrapped_conjunct_references("~[] Leaf /\\ Leaf")
        == []
    )
    assert (
        module.duplicate_zero_arity_wrapped_conjunct_references(
            "~([] Leaf) /\\ Leaf"
        )
        == []
    )
    assert module.duplicate_zero_arity_wrapped_conjunct_references(
        "~([] Leaf /\\ Leaf)"
    ) == ["Leaf"]
    assert (
        module.duplicate_zero_arity_wrapped_conjunct_references(
            "Leaf /\\ [] ~Leaf"
        )
        == []
    )
    assert module.contradictory_zero_arity_conjunct_references(
        "Leaf /\\ [] ~Leaf"
    ) == ["Leaf"]


def test_repeated_zero_arity_operands_descend_into_unary_temporal_operands() -> None:
    module = load_coverage_module()

    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf \\/ [] Leaf"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf => <> Leaf"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf <=> ~[] ~Leaf"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf => Leaf => Other"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf => Other => Leaf"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf <=> Leaf <=> Other"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf <=> Other <=> Leaf"
    ) == ["Leaf"]
    assert module.duplicate_zero_arity_boolean_operand_references(
        "Leaf <=> [] Leaf <=> Other"
    ) == ["Leaf"]
    assert (
        module.duplicate_zero_arity_boolean_operand_references("~[] Leaf \\/ Leaf")
        == []
    )
    assert (
        module.duplicate_zero_arity_boolean_operand_references(
            "~([] Leaf) \\/ Leaf"
        )
        == []
    )
    assert module.duplicate_zero_arity_boolean_operand_references(
        "~([] Leaf \\/ Leaf)"
    ) == ["Leaf"]
    assert (
        module.duplicate_zero_arity_boolean_operand_references("~[] Leaf <=> Leaf")
        == []
    )
    assert (
        module.duplicate_zero_arity_boolean_operand_references(
            "~([] Leaf) <=> Leaf"
        )
        == []
    )
    assert module.duplicate_zero_arity_boolean_operand_references(
        "~([] Leaf <=> Leaf)"
    ) == ["Leaf"]
    assert (
        module.duplicate_zero_arity_boolean_operand_references("Leaf \\/ [] ~Leaf")
        == []
    )


def test_quantified_formulas_descend_into_unary_temporal_exactness_wrappers() -> None:
    module = load_coverage_module()

    assert module.unary_temporal_quantified_formulas(
        "[] (\\A c \\in Cases: Predicate(c))"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.unary_temporal_quantified_formulas(
        "~[] (\\E c \\in Cases: Predicate(c))"
    ) == ["\\E c \\in Cases: Predicate(c)"]
    assert module.unary_temporal_quantified_formulas(
        "[] (Ready /\\ (\\A c \\in Cases: Predicate(c)))"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.unary_temporal_quantified_formulas(
        "[](\\A c \\in Cases: Left(c)) /\\ (\\A d \\in Other: Right(d))"
    ) == ["\\A c \\in Cases: Left(c)"]
    assert module.unary_temporal_quantified_formulas(
        "[]((\\A c \\in Cases: Left(c)) /\\ (\\A d \\in Other: Right(d)))"
    ) == [
        "\\A c \\in Cases: Left(c)",
        "\\A d \\in Other: Right(d)",
    ]
    assert module.unary_temporal_quantified_formulas(
        "[] (LET selected == \\A c \\in Cases: Predicate(c) IN selected)"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.unary_temporal_quantified_formulas(
        "<> (LET selected == \\A c \\in Cases: Predicate(c) IN ~selected)"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.unary_temporal_quantified_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []


def test_quantified_formulas_descend_into_static_wrappers() -> None:
    module = load_coverage_module()

    assert module.static_wrapped_quantified_formulas(
        "ENABLED (\\A c \\in Cases: Predicate(c))"
    ) == ["ENABLED wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "UNCHANGED <<\\E c \\in Cases: Predicate(c)>>"
    ) == ["UNCHANGED wraps \\E c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "SUBSET (\\A c \\in Cases: Predicate(c))"
    ) == ["SUBSET wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "CHOOSE x \\in Cases: \\A c \\in Other: Predicate(c)"
    ) == ["CHOOSE wraps \\A c \\in Other: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "LAMBDA x \\in Cases: \\A c \\in Other: Predicate(c)"
    ) == ["LAMBDA wraps \\A c \\in Other: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "Ready /\\ [] (ENABLED (\\A c \\in Cases: Predicate(c)))"
    ) == ["ENABLED wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "DOMAIN [x \\in Cases |-> \\A c \\in Other: Predicate(c)]"
    ) == ["DOMAIN wraps \\A c \\in Other: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "DOMAIN [Cases -> \\A c \\in Other: Predicate(c)]"
    ) == ["DOMAIN wraps \\A c \\in Other: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "SUBSET [Field: \\A c \\in Cases: Predicate(c)]"
    ) == ["SUBSET wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "SUBSET {x \\in Cases: \\A c \\in Other: Predicate(c)}"
    ) == ["SUBSET wraps \\A c \\in Other: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "UNCHANGED [record EXCEPT ![idx] = \\A c \\in Cases: Predicate(c)]"
    ) == ["UNCHANGED wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "ENABLED record[\\A c \\in Cases: Predicate(c)]"
    ) == ["ENABLED wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "ENABLED IF Ready THEN \\A c \\in Cases: Predicate(c) ELSE TRUE"
    ) == ["ENABLED wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "ENABLED Helper(\\A c \\in Cases: Predicate(c))"
    ) == ["ENABLED wraps \\A c \\in Cases: Predicate(c)"]
    assert module.static_wrapped_quantified_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []


def test_quantified_formulas_descend_into_structured_operands() -> None:
    module = load_coverage_module()

    assert module.structured_operand_quantified_formulas(
        "<<\\A c \\in Cases: Predicate(c)>>"
    ) == ["tuple literal contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "{\\A c \\in Cases: Predicate(c)}"
    ) == ["explicit set literal contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "{x \\in Cases: \\A c \\in Other: Predicate(c)}"
    ) == ["set comprehension contains \\A c \\in Other: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "[x \\in Cases |-> \\A c \\in Other: Predicate(c)]"
    ) == ["function constructor contains \\A c \\in Other: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "[Cases -> \\A c \\in Other: Predicate(c)]"
    ) == ["function set contains \\A c \\in Other: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "[Field |-> \\A c \\in Cases: Predicate(c)]"
    ) == ["record literal contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "[Field: \\A c \\in Cases: Predicate(c)]"
    ) == ["record set contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "[record EXCEPT ![idx] = \\A c \\in Cases: Predicate(c)]"
    ) == ["record update contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "record[\\A c \\in Cases: Predicate(c)]"
    ) == ["selector expression contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "IF Ready THEN \\A c \\in Cases: Predicate(c) ELSE TRUE"
    ) == ["IF expression contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "CASE Ready -> TRUE [] Other -> \\A c \\in Cases: Predicate(c)"
    ) == ["CASE expression contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "Ready = (\\A c \\in Cases: Predicate(c))"
    ) == ["relation expression contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "Known \\union (\\A c \\in Cases: Predicate(c))"
    ) == ["infix expression contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "Helper(\\A c \\in Cases: Predicate(c))"
    ) == ["operator call contains \\A c \\in Cases: Predicate(c)"]
    assert module.structured_operand_quantified_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []


def test_quantified_helper_formulas_descend_into_boolean_operands() -> None:
    module = load_coverage_module()

    assert module.quantified_helper_formulas(
        "ConcretePredicate /\\ (\\E c \\in Cases: Predicate(c))"
    ) == ["\\E c \\in Cases: Predicate(c)"]
    assert module.quantified_helper_formulas(
        "Ready => [] (\\A c \\in Cases: Predicate(c))"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.quantified_helper_formulas(
        "(\\A c \\in Cases: Predicate(c)) \\/ [] (\\A c \\in Cases: Predicate(c))"
    ) == ["\\A c \\in Cases: Predicate(c)"]
    assert module.quantified_helper_formulas(
        "[] ~(LET selected == \\E c \\in Cases: Predicate(c) IN selected)"
    ) == ["\\E c \\in Cases: Predicate(c)"]
    assert module.quantified_helper_formulas(
        "~(\\E c \\in Cases: Predicate(c))"
    ) == []


def test_quantified_helper_formulas_require_scoped_binding_prefixes() -> None:
    module = load_coverage_module()
    definitions = {
        "MalformedHelper": (
            1,
            "\\A MissingHelper \\subseteq MissingDomain: "
            "Predicate(MissingHelper)",
        ),
        "ScopedHelper": (2, "\\A c \\in Cases: Predicate(c)"),
    }
    relation_prefix = (
        "\\A MissingHelper \\subseteq MissingDomain: "
        "IF MissingHelper THEN Good ELSE Bad"
    )
    infix_prefix = (
        "\\A MissingHelper \\cup MissingDomain: "
        "IF MissingHelper THEN Good ELSE Bad"
    )
    existential_relation_prefix = (
        "\\E MissingHelper \\subseteq MissingDomain: Predicate(MissingHelper)"
    )

    assert module.is_scoped_quantified_formula("\\A c \\in Cases: Predicate(c)")
    assert module.is_scoped_quantified_formula("\\A c: Predicate(c)")
    assert not module.is_scoped_quantified_formula(relation_prefix)
    assert not module.is_scoped_quantified_formula(infix_prefix)
    assert module.is_quantified_helper_operand("ScopedHelper", definitions)
    assert not module.is_quantified_helper_operand("MalformedHelper", definitions)
    assert module.quantified_helper_formulas(relation_prefix) == []
    assert module.quantified_helper_formulas(infix_prefix) == []
    assert module.quantified_helper_formulas(
        f"[] (LET selected == {existential_relation_prefix} IN selected)"
    ) == []
    assert module.unary_temporal_quantified_formulas(
        f"[] ({existential_relation_prefix})"
    ) == []
    assert module.control_flow_quantified_helper_formulas(relation_prefix) == []
    assert module.control_flow_quantified_helper_formulas(infix_prefix) == []
    assert module.existential_quantified_helper_formulas(
        existential_relation_prefix
    ) == []
    assert module.negated_quantified_helper_formulas(f"~({relation_prefix})") == []


def test_vacuous_quantified_helper_formulas_detect_static_contradictions() -> None:
    module = load_coverage_module()

    assert module.quantified_formula_body("\\A c \\in Cases: Predicate(c)") == (
        "Predicate(c)"
    )
    assert module.tla_static_self_inequality("checked # checked") == (
        "checked # checked"
    )
    assert module.tla_static_self_inequality("checked /= checked") == (
        "checked /= checked"
    )
    assert module.temporal_self_inequality_parts(
        "[] (Predicate(c) \\/ checked # checked)"
    ) == ["checked # checked"]
    assert module.quantified_formula_body(
        '\\A c \\in Cases: label = "ready:yes" /\\ Predicate(c)'
    ) == 'label = "ready:yes" /\\ Predicate(c)'
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: TRUE"
    ) == ["\\A c \\in Cases: TRUE"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: checked = checked"
    ) == ["\\A c \\in Cases: checked = checked"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: checked # checked"
    ) == ["\\A c \\in Cases: checked # checked"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: checked /= checked"
    ) == ["\\A c \\in Cases: checked /= checked"]
    assert module.vacuous_quantified_helper_formulas(
        "[] (\\A c \\in Cases: [] (TRUE /\\ TRUE))"
    ) == ["\\A c \\in Cases: [] (TRUE /\\ TRUE)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: LET selected == TRUE IN selected"
    ) == ["\\A c \\in Cases: LET selected == TRUE IN selected"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: [] (LET selected == TRUE IN selected)"
    ) == ["\\A c \\in Cases: [] (LET selected == TRUE IN selected)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: LET first == TRUE second == first IN second"
    ) == ["\\A c \\in Cases: LET first == TRUE second == first IN second"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: [] (LET first == TRUE second == first IN second)"
    ) == ["\\A c \\in Cases: [] (LET first == TRUE second == first IN second)"]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: LET first == TRUE second == first /\\ TRUE IN second"
    ) == [
        "\\A c \\in Cases: LET first == TRUE second == first /\\ TRUE IN second"
    ]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: [] "
        "(LET first == TRUE second == first /\\ TRUE IN second)"
    ) == [
        "\\A c \\in Cases: [] "
        "(LET first == TRUE second == first /\\ TRUE IN second)"
    ]
    assert module.vacuous_quantified_helper_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []


def test_unused_bound_quantified_helper_formulas_require_body_use() -> None:
    module = load_coverage_module()

    assert module.quantified_formula_bound_identifiers(
        "\\A c \\in Cases: Predicate(c)"
    ) == {"c"}
    assert module.quantified_formula_bound_identifiers(
        "\\A c, d \\in Cases: Predicate(c, d)"
    ) == {"c", "d"}
    assert module.quantified_formula_bound_domains(
        "\\A c, d \\in Cases: Predicate(c, d)"
    ) == {"c": "Cases", "d": "Cases"}
    assert module.quantified_formula_bound_domains(
        "\\A c \\in Cases, d, e \\in Other: Predicate(c, d, e)"
    ) == {"c": "Cases", "d": "Other", "e": "Other"}
    assert module.quantified_formula_prefix_and_body(
        '\\A c \\in {"left \\" : right"}: Predicate(c)'
    ) == ('\\A c \\in {"left \\" : right"}', "Predicate(c)")
    assert module.quantified_formula_bound_domains(
        '\\A c \\in {"left \\" : right"}: Predicate(c)'
    ) == {"c": '{"left \\" : right"}'}
    assert not module.quantified_formula_has_empty_bound_domain(
        '\\A c \\in {"left \\" : right"}: Predicate(c)'
    )
    assert module.quantified_formula_prefix_and_body(
        "\\A c \\in <<Left :> Right>>: Predicate(c)"
    ) == ("\\A c \\in <<Left :> Right>>", "Predicate(c)")
    assert module.tla_quantifier_scope(
        "\\A c \\in <<Left :> Right>>: Predicate(c)"
    ) == (["<<Left :> Right>>"], "Predicate(c)", {"c"})
    assert module.quantified_formula_bound_domains(
        "\\A c \\in <<Left :> Right>>: Predicate(c)"
    ) == {"c": "<<Left :> Right>>"}
    assert module.quantified_formula_domain_expressions(
        "\\A c \\in <<Left :> Right>>: Predicate(c)"
    ) == ["<<Left :> Right>>"]
    assert module.quantified_helper_formulas(
        "\\A c \\in <<Left :> Right>>: Predicate(c)"
    ) == ["\\A c \\in <<Left :> Right>>: Predicate(c)"]
    assert module.tla_quantifier_prefixes(
        '\\A c \\in "left \\" : right": Predicate(c)'
    ) == ['\\A c \\in "left \\" : right"']
    assert module.tla_quantified_bound_identifiers(
        '\\A c \\in "left \\" : right": Predicate(c)'
    ) == {"c"}
    assert module.quantified_formula_bound_identifiers(
        '\\A c \\in "left \\" : right": Predicate(c)'
    ) == {"c"}
    assert module.quantified_formula_bound_domains(
        '\\A c \\in "left \\" : right": Predicate(c)'
    ) == {"c": '"left \\" : right"'}
    assert module.unused_bound_quantified_helper_formulas(
        '\\A c \\in "left \\" : right": Predicate(c)'
    ) == []
    assert module.unused_bound_quantified_helper_formulas(
        '\\A c \\in "left \\" : right": ConcretePredicate'
    ) == [
        '\\A c \\in "left \\" : right": ConcretePredicate omits bound c',
    ]
    assert module.quantified_formula_bound_domains(
        "\\A <<c, d>> \\in Pairs: Predicate(c, d)"
    ) == {}
    assert module.quantified_formula_domain_expressions(
        "\\A <<c, d>> \\in Pairs: Predicate(c, d)"
    ) == ["Pairs"]
    assert module.quantified_formula_bound_identifiers(
        "\\A <<c, d>> \\in Pairs: Predicate(c, d)"
    ) == {"c", "d"}
    assert module.quantified_formula_domain_expressions(
        "\\A c \\in Cases, <<d, e>> \\in Pairs: Predicate(c, d, e)"
    ) == ["Cases", "Pairs"]
    assert module.quantified_formula_bound_identifiers(
        "\\A c \\in Cases, <<d, e>> \\in Pairs: Predicate(c, d, e)"
    ) == {"c", "d", "e"}
    assert module.quantified_formula_bound_identifiers(
        "\\A c \\in Cases, d, e \\in Other: Predicate(c, d, e)"
    ) == {"c", "d", "e"}
    assert module.quantified_formula_bound_identifier_sequence(
        "\\A c, d \\in Cases: Predicate(c, d)"
    ) == ["c", "d"]
    assert module.quantified_formula_bound_identifier_sequence(
        "\\A c \\in Cases, <<d, e>> \\in Pairs: Predicate(c, d, e)"
    ) == ["c", "d", "e"]
    assert module.quantified_formula_duplicate_bound_identifiers(
        "\\A c, c \\in Cases: Predicate(c)"
    ) == ["c"]
    assert module.quantified_formula_duplicate_bound_identifiers(
        "\\A c \\in Cases, c \\in Other: Predicate(c)"
    ) == ["c"]
    assert module.quantified_formula_duplicate_bound_identifiers(
        "\\A <<c, c>> \\in Pairs: Predicate(c)"
    ) == ["c"]
    assert module.quantified_formula_duplicate_bound_identifiers(
        "\\A c, <<d, c>> \\in Pairs: Predicate(c, d)"
    ) == ["c"]
    assert module.duplicate_bound_quantified_helper_formulas(
        "\\A c, c \\in Cases: Predicate(c)"
    ) == ["\\A c, c \\in Cases: Predicate(c) duplicates bound c"]
    assert module.duplicate_bound_quantified_helper_formulas(
        "\\A c \\in Cases, <<d, c>> \\in Pairs: Predicate(c, d)"
    ) == [
        "\\A c \\in Cases, <<d, c>> \\in Pairs: Predicate(c, d) "
        "duplicates bound c"
    ]
    assert module.duplicate_bound_quantified_helper_formulas(
        "\\A c \\in Cases, d \\in Other: Predicate(c, d)"
    ) == []
    assert module.unused_bound_quantified_helper_formulas(
        "\\A c \\in Cases: ConcretePredicate"
    ) == ["\\A c \\in Cases: ConcretePredicate omits bound c"]
    assert module.unused_bound_quantified_helper_formulas(
        "\\A c: ConcretePredicate"
    ) == ["\\A c: ConcretePredicate omits bound c"]
    assert module.unused_bound_quantified_helper_formulas(
        "\\E c, d: Predicate(c)"
    ) == ["\\E c, d: Predicate(c) omits bound d"]
    assert module.unused_bound_quantified_helper_formulas("\\A c: Predicate(c)") == []
    assert module.unused_bound_quantified_helper_formulas(
        '\\A c \\in Cases: label = "c" /\\ ConcretePredicate'
    ) == ['\\A c \\in Cases: label = "c" /\\ ConcretePredicate omits bound c']
    assert module.unused_bound_quantified_helper_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []
    assert module.unused_bound_quantified_helper_formulas(
        "\\A c \\in Cases, d \\in Other: Predicate(c)"
    ) == ["\\A c \\in Cases, d \\in Other: Predicate(c) omits bound d"]
    assert module.unused_bound_quantified_helper_formulas(
        "\\A c \\in Cases, d, e \\in Other: Predicate(c)"
    ) == [
        "\\A c \\in Cases, d, e \\in Other: Predicate(c) omits bound d, e"
    ]
    assert module.unused_bound_quantified_helper_formulas(
        "\\A c \\in Cases, <<d, e>> \\in Pairs: Predicate(c, d)"
    ) == [
        "\\A c \\in Cases, <<d, e>> \\in Pairs: Predicate(c, d) omits bound e"
    ]
    assert module.unused_bound_quantified_helper_formulas(
        "\\A <<c, d>> \\in Pairs: Predicate(c)"
    ) == ["\\A <<c, d>> \\in Pairs: Predicate(c) omits bound d"]


def test_control_flow_quantified_helper_formulas_reject_predicate_selection() -> None:
    module = load_coverage_module()

    assert module.control_flow_quantified_helper_formulas(
        "\\A c \\in Cases: IF Ready(c) THEN Left(c) ELSE Right(c)"
    ) == [
        "\\A c \\in Cases: IF Ready(c) THEN Left(c) ELSE Right(c) uses IF"
    ]
    assert module.control_flow_quantified_helper_formulas(
        "\\A c \\in Cases: CASE Ready(c) -> Left(c) [] OTHER -> Right(c)"
    ) == [
        "\\A c \\in Cases: CASE Ready(c) -> Left(c) [] OTHER -> Right(c) uses CASE"
    ]
    assert module.control_flow_quantified_helper_formulas(
        "\\A c \\in Cases: LET selected == IF Ready(c) THEN Left(c) "
        "ELSE Right(c) IN selected"
    ) == [
        "\\A c \\in Cases: LET selected == IF Ready(c) THEN Left(c) "
        "ELSE Right(c) IN selected uses IF"
    ]
    assert module.control_flow_quantified_helper_formulas(
        "\\A c \\in Cases: LET local(x) == TRUE IN local(c)"
    ) == ["\\A c \\in Cases: LET local(x) == TRUE IN local(c) uses LET"]
    assert module.control_flow_quantified_helper_formulas(
        "\\A c \\in Cases: LET next == Next(c) IN next = Ready"
    ) == []
    assert module.control_flow_quantified_helper_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []


def test_negated_quantified_helper_formulas_reject_top_level_negation() -> None:
    module = load_coverage_module()

    assert module.negated_quantified_helper_formulas(
        "~(\\A c \\in Cases: Predicate(c))"
    ) == ["\\A c \\in Cases: Predicate(c) under 1 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "~~(\\A c \\in Cases: Predicate(c))"
    ) == ["\\A c \\in Cases: Predicate(c) under 2 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "\\A c \\in Cases: Predicate(c)"
    ) == []
    assert module.negated_quantified_helper_formulas(
        "ConcretePredicate /\\ ~(\\A c \\in Cases: Predicate(c))"
    ) == ["\\A c \\in Cases: Predicate(c) under 1 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "~(\\A c \\in Cases: Left(c)) /\\ (\\A d \\in Other: Right(d))"
    ) == ["\\A c \\in Cases: Left(c) under 1 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "~((\\A c \\in Cases: Left(c)) /\\ (\\A d \\in Other: Right(d)))"
    ) == [
        "\\A c \\in Cases: Left(c) under 1 top-level negation(s)",
        "\\A d \\in Other: Right(d) under 1 top-level negation(s)",
    ]
    assert module.negated_quantified_helper_formulas(
        "[] (~(\\A c \\in Cases: Predicate(c)))"
    ) == ["\\A c \\in Cases: Predicate(c) under 1 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "LET selected == \\A c \\in Cases: Predicate(c) IN ~selected"
    ) == ["\\A c \\in Cases: Predicate(c) under 1 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "~(LET selected == \\A c \\in Cases: Predicate(c) IN selected)"
    ) == ["\\A c \\in Cases: Predicate(c) under 1 top-level negation(s)"]
    assert module.negated_quantified_helper_formulas(
        "LET selected == \\A c \\in Cases: Predicate(c) IN selected"
    ) == []


def test_nested_control_flow_helper_formulas_require_named_helper_references() -> None:
    module = load_coverage_module()

    definitions = {
        "BugCollapsePredicate": (0, "Bug = Collapse"),
        "BranchPredicate": (4, "checked = branch"),
        "ConcretePredicate": (1, "checked = prepared"),
        "FallbackPredicate": (5, "checked = fallback"),
        "LeftPredicate": (2, "checked = ready"),
        "RightPredicate": (3, "ready = checked"),
    }

    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (IF ready THEN LeftPredicate ELSE RightPredicate)",
        definitions,
    ) == [("IF", "IF ready THEN LeftPredicate ELSE RightPredicate")]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (CASE ready -> LeftPredicate [] OTHER -> RightPredicate)",
        definitions,
    ) == [("CASE", "CASE ready -> LeftPredicate [] OTHER -> RightPredicate")]
    assert module.tla_top_level_case_result_branches(
        "CASE ready -> <<CASE nested -> LeftPredicate [] OTHER -> "
        "RightPredicate, tail>> [] OTHER -> FallbackPredicate"
    ) == [
        "<<CASE nested -> LeftPredicate [] OTHER -> RightPredicate, tail>>",
        "FallbackPredicate",
    ]
    assert module.tla_top_level_case_result_branches(
        'CASE ready -> "Left \\" [] Right" [] OTHER -> FallbackPredicate'
    ) == ['"Left \\" [] Right"', "FallbackPredicate"]
    assert module.tla_top_level_case_result_branches(
        "CASE <<CASE nested -> LeftPredicate [] OTHER -> RightPredicate, "
        "tail>> = expected -> BranchPredicate [] OTHER -> FallbackPredicate"
    ) == ["BranchPredicate", "FallbackPredicate"]
    assert module.tla_top_level_case_result_branches(
        'CASE "ready \\" -> text" = expected -> BranchPredicate [] '
        "OTHER -> FallbackPredicate"
    ) == ["BranchPredicate", "FallbackPredicate"]
    assert module.control_flow_named_helper_branch_operands(
        "CASE <<CASE nested -> LeftPredicate [] OTHER -> RightPredicate, "
        "tail>> = expected -> BranchPredicate [] OTHER -> FallbackPredicate",
        definitions,
    ) == ["BranchPredicate", "FallbackPredicate"]
    assert module.tla_top_level_case_result_branches(
        "CASE ready -> [] LeftPredicate [] OTHER -> RightPredicate"
    ) == ["[] LeftPredicate", "RightPredicate"]
    assert module.control_flow_named_helper_branch_operands(
        "CASE ready -> [] LeftPredicate [] OTHER -> RightPredicate",
        definitions,
    ) == ["LeftPredicate", "RightPredicate"]
    assert module.tla_top_level_case_result_branches(
        "CASE [] ReadyPredicate -> LeftPredicate [] OTHER -> RightPredicate"
    ) == ["LeftPredicate", "RightPredicate"]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (IF ready THEN LET selected == LeftPredicate IN "
        "selected ELSE LET fallback == RightPredicate IN fallback)",
        definitions,
    ) == [
        (
            "IF",
            "IF ready THEN LET selected == LeftPredicate IN selected ELSE "
            "LET fallback == RightPredicate IN fallback",
        )
    ]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (CASE ready -> LET selected == LeftPredicate IN "
        "selected [] OTHER -> LET fallback == RightPredicate IN fallback)",
        definitions,
    ) == [
        (
            "CASE",
            "CASE ready -> LET selected == LeftPredicate IN selected [] "
            "OTHER -> LET fallback == RightPredicate IN fallback",
        )
    ]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (LET selected == IF ready THEN LeftPredicate "
        "ELSE RightPredicate IN selected)",
        definitions,
    ) == [("IF", "IF ready THEN LeftPredicate ELSE RightPredicate")]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ ~(LET selected == CASE ready -> LeftPredicate "
        "[] OTHER -> RightPredicate IN selected)",
        definitions,
    ) == [("CASE", "CASE ready -> LeftPredicate [] OTHER -> RightPredicate")]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (ENABLED LeftPredicate)",
        definitions,
    ) == [("ENABLED", "ENABLED LeftPredicate")]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (CHOOSE c \\in Cases: LeftPredicate)",
        definitions,
    ) == [("CHOOSE", "CHOOSE c \\in Cases: LeftPredicate")]
    assert module.control_flow_named_helper_branch_operands(
        'ENABLED "LeftPredicate"',
        definitions,
    ) == []
    assert module.control_flow_named_helper_branch_operands(
        'ENABLED (LeftPredicate /\\ "RightPredicate")',
        definitions,
    ) == ["LeftPredicate"]
    assert module.control_flow_named_helper_branch_operands(
        'CHOOSE c \\in Cases: "LeftPredicate"',
        definitions,
    ) == []
    assert module.control_flow_named_helper_branch_operands(
        'CHOOSE c \\in Cases: LeftPredicate /\\ "RightPredicate"',
        definitions,
    ) == ["LeftPredicate"]
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (IF ready THEN LET selected == LeftPredicate IN "
        "[] selected ELSE LET fallback == RightPredicate IN <> fallback)",
        definitions,
    ) == [
        (
            "IF",
            "IF ready THEN LET selected == LeftPredicate IN [] selected ELSE "
            "LET fallback == RightPredicate IN <> fallback",
        )
    ]
    assert module.nested_control_flow_helper_formulas(
        "IF ready THEN LeftPredicate ELSE RightPredicate",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (IF ready THEN TRUE ELSE ready)",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (IF ready THEN ~BugCollapsePredicate ELSE TRUE)",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (ENABLED BugCollapsePredicate)",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (ENABLED TRUE)",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (UNCHANGED LeftPredicate)",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "ConcretePredicate /\\ (IF ready THEN LET selected == BugCollapsePredicate "
        "IN selected ELSE LET fallback == TRUE IN fallback)",
        definitions,
    ) == []
    assert module.nested_control_flow_helper_formulas(
        "[] (IF ready THEN LeftPredicate ELSE RightPredicate)",
        definitions,
    ) == []
    assert (
        module.exactness_boolean_helper_operand_name(
            "LET selected == [] ~LeftPredicate IN selected"
        )
        == "LeftPredicate"
    )
    assert (
        module.exactness_boolean_helper_operand_name(
            "LET selected == LeftPredicate IN [] ~selected"
        )
        == "LeftPredicate"
    )


def test_exactness_boolean_composition_kinds_descend_into_boolean_operands() -> None:
    module = load_coverage_module()

    definitions = {
        "ConcretePredicate": (1, "checked = prepared"),
        "LeftPredicate": (2, "checked = ready"),
        "RightPredicate": (3, "ready = checked"),
        "LeftCall": (4, "ParameterizedPredicate(Cases)"),
        "RightCall": (5, "OtherPredicate(Cases)"),
        "LeftQuantified": (6, "\\A c \\in Cases: LeftPredicate(c)"),
        "RightQuantified": (7, "\\A c \\in Cases: RightPredicate(c)"),
        "LeftComparisonCall": (8, "ParameterizedPredicate(ready = checked)"),
        "RightComparisonCall": (9, "OtherPredicate(ready = checked)"),
    }
    signatures = {
        "ParameterizedPredicate": (10, 1),
        "OtherPredicate": (11, 1),
    }

    assert module.exactness_helper_boolean_composition_kind(
        "ConcretePredicate /\\ (LeftPredicate \\/ RightPredicate)",
        definitions,
    ) == "disjunction"
    assert module.exactness_helper_boolean_composition_kind(
        "ConcretePredicate /\\ (LeftPredicate => RightPredicate)",
        definitions,
    ) == "implication"
    assert module.exactness_helper_boolean_composition_kind(
        "ConcretePredicate /\\ (LeftPredicate <=> RightPredicate)",
        definitions,
    ) == "equivalence"
    assert module.exactness_helper_boolean_composition_kind(
        "ConcretePredicate /\\ "
        "(LET selected == LeftPredicate \\/ RightPredicate IN selected)",
        definitions,
    ) == "disjunction"
    assert module.exactness_helper_boolean_composition_kind(
        "TRUE /\\ ~LeftPredicate",
        definitions,
    ) == "negation"
    assert module.exactness_helper_boolean_composition_kind(
        "FALSE \\/ ~LeftPredicate",
        definitions,
    ) == "negation"
    assert module.exactness_helper_boolean_composition_kind(
        "TRUE => ~LeftPredicate",
        definitions,
    ) == "negation"
    assert module.exactness_helper_boolean_composition_kind(
        "TRUE <=> ~LeftPredicate",
        definitions,
    ) == "negation"
    assert module.exactness_helper_boolean_composition_kind(
        "ConcretePredicate /\\ ~LeftPredicate",
        definitions,
    ) is None
    assert module.exactness_parameterized_call_boolean_composition_kind(
        "ConcretePredicate /\\ (LeftCall \\/ RightCall)",
        definitions,
        signatures,
    ) == "disjunction"
    assert module.exactness_parameterized_call_boolean_composition_kind(
        "ConcretePredicate /\\ (LET selected == LeftCall \\/ RightCall IN selected)",
        definitions,
        signatures,
    ) == "disjunction"
    assert module.exactness_parameterized_call_boolean_composition_kind(
        "ConcretePredicate /\\ (LeftComparisonCall \\/ RightComparisonCall)",
        definitions,
        signatures,
    ) == "disjunction"
    assert module.exactness_parameterized_call_boolean_composition_kind(
        "TRUE /\\ ~LeftCall",
        definitions,
        signatures,
    ) == "negation"
    assert module.exactness_quantified_boolean_composition_kind(
        "ConcretePredicate /\\ (LeftQuantified \\/ RightQuantified)",
        definitions,
        signatures,
    ) == "disjunction"
    assert module.exactness_quantified_boolean_composition_kind(
        "ConcretePredicate /\\ "
        "(LET selected == LeftQuantified \\/ RightQuantified IN selected)",
        definitions,
        signatures,
    ) == "disjunction"
    assert module.exactness_quantified_boolean_composition_kind(
        "TRUE /\\ ~LeftQuantified",
        definitions,
        signatures,
    ) == "negation"


def test_direct_operator_call_name_accepts_expression_arguments() -> None:
    module = load_coverage_module()

    assert module.tla_top_level_argument_parts("<<1, 2>>, <<3, 4>>") == [
        "<<1, 2>>",
        "<<3, 4>>",
    ]
    assert module.tla_top_level_conjuncts("<<Left /\\ Right, Tail>>") == [
        "<<Left /\\ Right, Tail>>"
    ]
    assert module.tla_top_level_conjuncts('"Left \\" /\\ Right" /\\ Tail') == [
        '"Left \\" /\\ Right"',
        "Tail",
    ]
    assert module.tla_top_level_disjuncts("<<Left \\/ Right, Tail>>") == [
        "<<Left \\/ Right, Tail>>"
    ]
    assert module.tla_top_level_disjuncts('"Left \\" \\/ Right" \\/ Tail') == [
        '"Left \\" \\/ Right"',
        "Tail",
    ]
    assert not module.tla_has_top_level_disjunction("<<Left \\/ Right, Tail>>")
    assert module.tla_top_level_implication_operands(
        "<<Left => Right, Tail>>"
    ) == ["<<Left => Right, Tail>>"]
    assert module.tla_top_level_implication_operands(
        '"Left \\" => Right" => Tail'
    ) == ['"Left \\" => Right"', "Tail"]
    assert module.tla_top_level_implication_chain_operands(
        "<<Left => Right, Tail>>"
    ) == ["<<Left => Right, Tail>>"]
    assert module.tla_top_level_implication_chain_operands("Left => Right => Tail") == [
        "Left",
        "Right",
        "Tail",
    ]
    assert module.tla_top_level_implication_chain_operands(
        '"Left \\" => Right" => Tail => Done'
    ) == ['"Left \\" => Right"', "Tail", "Done"]
    assert not module.tla_has_top_level_implication("<<Left => Right, Tail>>")
    assert module.tla_top_level_equivalence_operands(
        "<<Left <=> Right, Tail>>"
    ) == ["<<Left <=> Right, Tail>>"]
    assert module.tla_top_level_equivalence_operands(
        '"Left \\" <=> Right" <=> Tail'
    ) == ['"Left \\" <=> Right"', "Tail"]
    assert module.tla_top_level_equivalence_chain_operands(
        "<<Left <=> Right, Tail>>"
    ) == ["<<Left <=> Right, Tail>>"]
    assert module.tla_top_level_equivalence_chain_operands(
        "Left <=> Right <=> Tail"
    ) == ["Left", "Right", "Tail"]
    assert module.tla_top_level_equivalence_chain_operands(
        '"Left \\" <=> Right" <=> Tail <=> Done'
    ) == ['"Left \\" <=> Right"', "Tail", "Done"]
    assert module.tla_top_level_conjuncts("Guard /\\ <<Left /\\ Right, Tail>>") == [
        "Guard",
        "<<Left /\\ Right, Tail>>",
    ]
    assert module.tla_top_level_membership_parts("<<c \\in Cases, tail>>") is None
    assert module.tla_top_level_membership_parts("Left \\subseteq Right") is None
    assert module.tla_top_level_subset_relation_parts("Left \\subseteq Right") == (
        "Left",
        "\\subseteq",
        "Right",
    )
    assert module.tla_top_level_relation_parts("Left \\subseteq Right") == (
        "Left",
        "\\subseteq",
        "Right",
    )
    assert (
        module.tla_top_level_equality_relation_parts("<<c = c, tail>>") is None
    )
    assert module.tla_top_level_equality_relation_parts("Left = Right") == (
        "Left",
        "=",
        "Right",
    )
    for wrapped_relation in (
        "IF Left = Right THEN Good ELSE Bad",
        "CASE Left = Right -> Good [] OTHER -> Bad",
        "LET selected == Left = Right IN selected",
        "CHOOSE Left \\in Cases: Left = Right",
        "ENABLED Left = Right",
        "UNCHANGED Left = Right",
        "[] Left = Right",
        "<> Left = Right",
        "~Left = Right",
    ):
        assert module.tla_top_level_equality_relation_parts(wrapped_relation) is None
        assert module.tla_top_level_relation_parts(wrapped_relation) is None
        assert module.tla_top_level_relation_operator(wrapped_relation) is None
        assert not module.tla_has_top_level_equality(wrapped_relation)
    assert module.tla_top_level_order_relation_parts("Left => Right") is None
    assert module.tla_top_level_order_relation_parts("Left <=> Right") is None
    assert module.tla_top_level_order_relation_parts("<<c < d, tail>>") is None
    assert module.tla_top_level_static_infix_operands("Left + Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands(
        "Left \\cup {Right, Tail}"
    ) == ["Left", "{Right, Tail}"]
    assert module.tla_top_level_static_infix_operands("Left \\union Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left \\intersect Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left \\setminus Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left \\cdot Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left \\o Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left @@ Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left :> Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands("Left ^ Right") == [
        "Left",
        "Right",
    ]
    assert module.tla_top_level_static_infix_operands(
        "<<Left + Right, Tail>>"
    ) is None
    assert module.tla_top_level_static_infix_operands("-Left") is None
    assert module.tla_top_level_static_infix_operands("Left -> Right") is None
    assert not module.tla_has_top_level_equality("<<c = c, tail>>")
    assert module.tla_has_top_level_disjunction("Left \\/ Right")
    assert module.tla_has_top_level_implication("Left => Right")
    assert module.tla_has_top_level_equality("left = right")
    assert (
        module.strip_static_outer_parentheses('("left \\" ) right")')
        == '"left \\" ) right"'
    )
    assert (
        module.strip_static_outer_parentheses('("left ( right")')
        == '"left ( right"'
    )
    assert (
        module.tla_static_let_alias_operand(
            'LET selected == ("left \\" ) right") IN selected'
        )
        == '"left \\" ) right"'
    )
    assert module.tla_explicit_set_elements("{<<1, 2>>}") == ["<<1, 2>>"]
    assert module.tla_explicit_singleton_set_element("{<<1, 2>>}") == "<<1, 2>>"
    assert module.tla_top_level_if_parts(
        "IF ready THEN <<left ELSE right, tail>> ELSE fallback"
    ) == ("ready", "<<left ELSE right, tail>>", "fallback")
    assert module.tla_top_level_if_parts(
        'IF ready THEN "left \\" ELSE right" ELSE fallback'
    ) == ("ready", '"left \\" ELSE right"', "fallback")
    assert module.tla_top_level_if_parts(
        'IF "guard \\" THEN text" THEN left ELSE right'
    ) == ('"guard \\" THEN text"', "left", "right")
    assert (
        module.tla_static_let_alias_operand(
            "LET selected == <<left IN right, tail>> IN selected"
        )
        == "<<left IN right, tail>>"
    )
    assert (
        module.tla_direct_operator_call_name("Predicate(ready = checked)")
        == "Predicate"
    )
    assert (
        module.tla_direct_operator_call_name(
            'Predicate(IF ready THEN <<checked, "literal )">> ELSE <<>>)'
        )
        == "Predicate"
    )
    assert module.tla_direct_operator_call_name("Predicate(Cases) /\\ Other") is None
    assert module.tla_direct_operator_call_name("IF(TRUE)") is None


def test_control_flow_formulas_descend_into_unary_temporal_wrappers() -> None:
    module = load_coverage_module()

    assert module.unary_temporal_control_flow_formulas(
        "[] (IF TRUE THEN Left ELSE Right)"
    ) == [("IF", "IF TRUE THEN Left ELSE Right")]
    assert module.unary_temporal_control_flow_formulas(
        "~[] (CASE TRUE -> Left [] OTHER -> Right)"
    ) == [("CASE", "CASE TRUE -> Left [] OTHER -> Right")]
    assert module.unary_temporal_control_flow_formulas(
        "[] (LET selected == IF TRUE THEN Left ELSE Right IN selected)"
    ) == [("IF", "IF TRUE THEN Left ELSE Right")]
    assert module.unary_temporal_control_flow_formulas(
        "LET selected == IF TRUE THEN Left ELSE Right IN [] selected"
    ) == [("IF", "IF TRUE THEN Left ELSE Right")]
    assert module.unary_temporal_control_flow_formulas(
        "ConcretePredicate /\\ "
        "(LET selected == IF TRUE THEN Left ELSE Right IN [] selected)"
    ) == [("IF", "IF TRUE THEN Left ELSE Right")]
    assert module.unary_temporal_control_flow_formulas(
        "[](CASE TRUE -> Left [] OTHER -> Right) /\\ "
        "CASE TRUE -> Outside [] OTHER -> Fallback"
    ) == [("CASE", "CASE TRUE -> Left [] OTHER -> Right")]
    assert module.unary_temporal_control_flow_formulas(
        "[]((CASE TRUE -> Left [] OTHER -> Right) /\\ "
        "(CASE TRUE -> Outside [] OTHER -> Fallback))"
    ) == [
        ("CASE", "CASE TRUE -> Left [] OTHER -> Right"),
        ("CASE", "CASE TRUE -> Outside [] OTHER -> Fallback"),
    ]
    assert module.unary_temporal_control_flow_formulas(
        "[] (IF TRUE THEN TRUE ELSE FALSE)"
    ) == []
    assert module.unary_temporal_control_flow_formulas(
        "IF TRUE THEN Left ELSE Right"
    ) == []


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
                "  RECURSIVE Scoped(_)",
                "RECURSIVE",
                "  RECURSIVE",
                "RECURSIVEFoo",
                "RECURSIVEFoo == TRUE",
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
        f"frontier-fast: {tla}:5 RECURSIVE declaration directive must be "
        "top-level: RECURSIVE Scoped(_)",
        f"frontier-fast: {tla}:6 RECURSIVE declaration must list static "
        "operator declarations: RECURSIVE",
        f"frontier-fast: {tla}:7 RECURSIVE declaration directive must be "
        "top-level: RECURSIVE",
        f"frontier-fast: {tla}:8 malformed RECURSIVE declaration directive "
        "RECURSIVE: RECURSIVEFoo",
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
                "INSTANCE Imported",
                "Alias == INSTANCE Named",
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
        ],
        [],
    )


def test_tla_instance_alias_entries_parse_valid_named_instances(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "INSTANCE Imported",
                "LOCAL INSTANCE LocalImported",
                "Alias == INSTANCE Named",
                "LOCAL LocalAlias == INSTANCE Hidden",
                "Configured == INSTANCE WithConfig WITH Foo <- Bar",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_instance_alias_entries(tla) == [
        (4, "Alias"),
    ]


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
                "EXTENDS",
                "EXTENDSNaturals",
                "INSTANCE Imported",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(8, "INSTANCE", "Imported")],
        [
            f"{tla}:2 EXTENDS must list static module identifiers: Naturals,,FiniteSets",
            f"{tla}:3 EXTENDS must list static module identifiers: Naturals + Hidden",
            f"{tla}:4 EXTENDS must list static module identifiers: Naturals,",
            f"{tla}:5 EXTENDS must list non-reserved static module identifiers: TRUE",
            f"{tla}:6 EXTENDS must list static module identifiers: EXTENDS",
            f"{tla}:7 malformed EXTENDS dependency declaration: EXTENDSNaturals",
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
                "INSTANCEImported",
                "LOCALINSTANCE LocalImported",
                "LOCAL INSTANCELocalImported",
                "INSTANCE 123Bad",
                "Alias == INSTANCE Named + Hidden",
                "LOCAL INSTANCE",
                "LOCAL INSTANCE LocalImported",
                "LOCAL LocalAlias == INSTANCE Named",
                "INSTANCE TRUE",
                "Alias == INSTANCE WF_Module",
                "TRUE == INSTANCE Imported",
                "WF_Alias == INSTANCE Imported",
                "Alias == INSTANCEImported",
                "LOCAL LocalAlias == INSTANCEImported",
                "INSTANCE Imported",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(16, "INSTANCE", "Imported")],
        [
            f"{tla}:2 malformed INSTANCE dependency declaration: INSTANCEImported",
            f"{tla}:3 malformed INSTANCE dependency declaration: LOCALINSTANCE LocalImported",
            f"{tla}:4 malformed INSTANCE dependency declaration: LOCAL INSTANCELocalImported",
            f"{tla}:5 INSTANCE must reference a static module identifier: INSTANCE 123Bad",
            f"{tla}:6 INSTANCE must reference a static module identifier: "
            "Alias == INSTANCE Named + Hidden",
            f"{tla}:7 INSTANCE must reference a static module identifier: LOCAL INSTANCE",
            f"{tla}:8 INSTANCE declarations must be non-LOCAL: "
            "LOCAL INSTANCE LocalImported",
            f"{tla}:9 INSTANCE declarations must be non-LOCAL: "
            "LOCAL LocalAlias == INSTANCE Named",
            f"{tla}:10 INSTANCE must reference a non-reserved static module "
            "identifier: INSTANCE TRUE",
            f"{tla}:11 INSTANCE must reference a non-reserved static module "
            "identifier: Alias == INSTANCE WF_Module",
            f"{tla}:12 INSTANCE alias must be a non-reserved static identifier: "
            "TRUE == INSTANCE Imported",
            f"{tla}:13 INSTANCE alias must be a non-reserved static identifier: "
            "WF_Alias == INSTANCE Imported",
            f"{tla}:14 malformed INSTANCE dependency declaration: "
            "Alias == INSTANCEImported",
            f"{tla}:15 malformed INSTANCE dependency declaration: "
            "LOCAL LocalAlias == INSTANCEImported",
        ],
    )


def test_tla_module_dependency_references_rejects_instance_substitutions(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "INSTANCE Imported WITH Foo <- Bar",
                "Configured == INSTANCE WithConfig WITH Foo <- Bar",
                "INSTANCE Plain",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(4, "INSTANCE", "Plain")],
        [
            f"{tla}:2 INSTANCE substitutions are not supported; use a static "
            "module identifier without WITH: INSTANCE Imported WITH Foo <- Bar",
            f"{tla}:3 INSTANCE substitutions are not supported; use a static "
            "module identifier without WITH: Configured == INSTANCE "
            "WithConfig WITH Foo <- Bar",
        ],
    )


def test_tla_module_dependency_references_rejects_malformed_instance_aliases(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "Bad-Alias == INSTANCE Named",
                "123Alias == INSTANCE Named",
                "LOCAL Bad-Alias == INSTANCE Named",
                "INSTANCE Plain",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(5, "INSTANCE", "Plain")],
        [
            f"{tla}:2 INSTANCE alias must be a non-reserved static "
            "identifier: Bad-Alias == INSTANCE Named",
            f"{tla}:3 INSTANCE alias must be a non-reserved static "
            "identifier: 123Alias == INSTANCE Named",
            f"{tla}:4 INSTANCE declarations must be non-LOCAL: "
            "LOCAL Bad-Alias == INSTANCE Named",
        ],
    )


def test_tla_module_dependency_references_rejects_indented_declarations(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "  EXTENDS Hidden",
                "  EXTENDS",
                "  INSTANCE Imported",
                "  Alias == INSTANCE Named",
                "  Bad-Alias == INSTANCE Named",
                "INSTANCE Plain",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(7, "INSTANCE", "Plain")],
        [
            f"{tla}:2 TLA dependency declarations must be top-level: "
            "EXTENDS Hidden",
            f"{tla}:3 TLA dependency declarations must be top-level: "
            "EXTENDS",
            f"{tla}:4 TLA dependency declarations must be top-level: "
            "INSTANCE Imported",
            f"{tla}:5 TLA dependency declarations must be top-level: "
            "Alias == INSTANCE Named",
            f"{tla}:6 TLA dependency declarations must be top-level: "
            "Bad-Alias == INSTANCE Named",
        ],
    )


def test_tla_module_dependency_references_rejects_late_declarations(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS Naturals",
                "VARIABLE state",
                "EXTENDS LateHelpers",
                "Alias == INSTANCE LateNamed",
                "Helper == TRUE",
                "INSTANCE LateBare",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_dependency_references(tla) == (
        [(2, "EXTENDS", "Naturals")],
        [
            f"{tla}:4 TLA dependency declarations must appear before "
            "declarations and definitions: EXTENDS LateHelpers",
            f"{tla}:5 TLA dependency declarations must appear before "
            "declarations and definitions: Alias == INSTANCE LateNamed",
            f"{tla}:7 TLA dependency declarations must appear before "
            "declarations and definitions: INSTANCE LateBare",
        ],
    )


def test_tla_instance_alias_namespace_errors_rejects_duplicates_and_overlaps(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANT ConfigAlias",
                "VARIABLE state",
                "vars == <<state>>",
                "Helper == TRUE",
                "ConfigAlias == INSTANCE ConfigModule",
                "state == INSTANCE StateModule",
                "Helper == INSTANCE HelperModule",
                "Alias == INSTANCE First",
                "Alias == INSTANCE Second",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_instance_alias_namespace_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:6 INSTANCE alias ConfigAlias overlaps with "
        "constant declaration at line 2",
        f"frontier-fast: {tla}:7 INSTANCE alias state overlaps with "
        "variable declaration at line 3",
        f"frontier-fast: {tla}:8 INSTANCE alias Helper overlaps with "
        "TLA operator definition at line 5",
        f"frontier-fast: {tla}:10 repeats INSTANCE alias Alias first "
        "declared at line 9",
    ]


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


def test_tla_module_dependency_errors_rejects_duplicate_dependencies(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    helper = tmp_path / "LocalHelpers.tla"
    missing = tmp_path / "MissingHelpers.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS Naturals, LocalHelpers, Naturals",
                "INSTANCE LocalHelpers",
                "INSTANCE MissingHelpers",
                "INSTANCE MissingHelpers",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    helper.write_text("---- MODULE LocalHelpers ----\n====\n", encoding="utf-8")

    assert module.tla_module_dependency_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 repeats TLA module dependency Naturals "
        "first referenced as EXTENDS at line 2",
        f"frontier-fast: {tla}:3 repeats TLA module dependency LocalHelpers "
        "first referenced as EXTENDS at line 2",
        f"frontier-fast: {tla}:4 references INSTANCE module MissingHelpers, "
        f"but neither TLA standard module nor {missing} exists",
        f"frontier-fast: {tla}:5 repeats TLA module dependency MissingHelpers "
        "first referenced as INSTANCE at line 4",
    ]


def test_tla_reachable_module_files_returns_transitive_local_dependencies(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    root = tmp_path / "Model.tla"
    helper = tmp_path / "LocalHelpers.tla"
    shared = tmp_path / "SharedHelpers.tla"
    root.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS Naturals, LocalHelpers, MissingHelpers",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    helper.write_text(
        "\n".join(
            [
                "---- MODULE LocalHelpers ----",
                "EXTENDS SharedHelpers, Naturals",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    shared.write_text(
        "\n".join(
            [
                "---- MODULE SharedHelpers ----",
                "EXTENDS LocalHelpers",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_local_dependency_files(root) == (helper,)
    assert module.tla_reachable_module_files(root) == (root, helper, shared)


def test_tla_module_validation_errors_check_reachable_dependency_modules(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    root = tmp_path / "Model.tla"
    helper = tmp_path / "LocalHelpers.tla"
    root.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "EXTENDS LocalHelpers",
                "VARIABLE state",
                "vars == <<state>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    helper.write_text(
        "\n".join(
            [
                "---- MODULE DriftedHelpers ----",
                "VARIABLE state",
                "vars == <<state>>",
                "AXIOM FALSE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_module_validation_errors("frontier-fast", root) == [
        f"frontier-fast: {helper} declares MODULE DriftedHelpers, "
        "expected LocalHelpers",
        f"frontier-fast: {helper}:4 uses top-level AXIOM directive; "
        "Sumeragi formal modules must be assumption-free",
    ]
    assert module.tla_module_validation_errors("frontier-deep", root) == [
        f"frontier-deep: {helper} declares MODULE DriftedHelpers, "
        "expected LocalHelpers",
        f"frontier-deep: {helper}:4 uses top-level AXIOM directive; "
        "Sumeragi formal modules must be assumption-free",
    ]


def test_tla_forbidden_directive_errors_rejects_assumptions_and_proofs(
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
                "ASSUMEFALSE",
                "  THEOREMProof",
                "ASSUMEFALSE == TRUE",
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
        f"frontier-fast: {tla}:8 uses indented ASSUME directive; "
        "Sumeragi formal modules must be assumption-free",
        f"frontier-fast: {tla}:9 uses indented THEOREM directive; "
        "Sumeragi formal modules must be proof-free",
        f"frontier-fast: {tla}:10 uses top-level ASSUME directive start: "
        "ASSUMEFALSE; Sumeragi formal modules must be assumption-free",
        f"frontier-fast: {tla}:11 uses indented THEOREM directive start: "
        "THEOREMProof; Sumeragi formal modules must be proof-free",
    ]
    assert module.tla_operator_definitions(tla) == {"ASSUMEFALSE"}


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
                "CONSTANT, Hidden = TRUE",
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
            f"{cfg}:3 directive CONSTANT, must bind exactly one constant",
        ],
    )


def test_cfg_constant_bindings_rejects_no_separator_directive_starts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "NoSeparatorConstants.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANTSMaxView = 2",
                "CONSTANTBug = TRUE",
                "CONSTANTS",
                "  CONSTANTS_MAX = 3",
                "  CONSTANT_READY <- Bool",
                "INIT Init",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(4, "CONSTANTS_MAX"), (5, "CONSTANT_READY")],
        [
            f"{cfg}:1 malformed CFG constant binding directive CONSTANTS: "
            "CONSTANTSMaxView = 2",
            f"{cfg}:2 malformed CFG constant binding directive CONSTANT: "
            "CONSTANTBug = TRUE",
        ],
    )


def test_cfg_constant_bindings_rejects_indented_no_separator_directive_starts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "IndentedNoSeparatorConstants.cfg"
    cfg.write_text(
        "\n".join(
            [
                "CONSTANTS",
                "  CONSTANTS_MAX = 3",
                "  CONSTANTSMax = 4",
                "CONSTANTS",
                "  CONSTANT_READY <- Bool",
                "  CONSTANTFlag = TRUE",
                "INIT Init",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(2, "CONSTANTS_MAX"), (5, "CONSTANT_READY")],
        [
            f"{cfg}:3 indented CFG directive CONSTANTS must be top-level",
            f"{cfg}:6 indented CFG directive CONSTANT must be top-level",
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
                "  CONSTANT, Hidden = TRUE",
                "  Toggle <- Bool",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_constant_bindings(cfg) == (
        [(2, "MaxView")],
        [
            f"{cfg}:3 indented CFG directive INVARIANT must be top-level",
            f"{cfg}:4 indented CFG directive CONSTANT, must be top-level",
        ],
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
                "  Toggle,",
                "  CONSTANTSValue,",
                "  CONSTANTFlag",
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
        "CONSTANTFlag",
        "CONSTANTSValue",
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
                "CONSTANTSMaxView",
                "CONSTANTBug",
                "CONSTANTS Good, , Bad",
                "CONSTANT, Hidden",
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
        f"frontier-fast: {tla}:2 malformed CONSTANTS declaration directive "
        "CONSTANTS: CONSTANTSMaxView",
        f"frontier-fast: {tla}:3 malformed CONSTANTS declaration directive "
        "CONSTANT: CONSTANTBug",
        f"frontier-fast: {tla}:4 CONSTANTS declaration line must list static "
        "identifiers: Good, , Bad",
        f"frontier-fast: {tla}:5 CONSTANTS declaration line must list static "
        "identifiers: CONSTANT, Hidden",
        f"frontier-fast: {tla}:7 CONSTANTS declaration block ends with "
        "trailing comma",
    ]


def test_tla_duplicate_constant_declaration_errors_rejects_indented_directives(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "  CONSTANTS Hidden",
                "  CONSTANT, Hidden",
                "CONSTANTS",
                "  Visible",
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
        f"frontier-fast: {tla}:2 CONSTANTS declaration directive must "
        "be top-level: CONSTANTS Hidden",
        f"frontier-fast: {tla}:3 CONSTANTS declaration directive must "
        "be top-level: CONSTANT, Hidden",
    ]
    assert module.tla_constant_declarations(tla) == {"Visible"}


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


def test_tla_declaration_blocks_reject_top_level_no_separator_entries(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS",
                "CONSTANTSValue",
                "CONSTANTFlag",
                "CONSTANTS",
                "  Good",
                "VARIABLES",
                "VARIABLESState",
                "VARIABLEFlag",
                "VARIABLES",
                "  checked",
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
        f"frontier-fast: {tla}:3 malformed CONSTANTS declaration directive "
        "CONSTANTS: CONSTANTSValue",
        f"frontier-fast: {tla}:4 malformed CONSTANTS declaration directive "
        "CONSTANT: CONSTANTFlag",
    ]
    assert module.tla_constant_declarations(tla) == {"Good"}
    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:7 VARIABLES block must declare at least one identifier",
        f"frontier-fast: {tla}:8 malformed VARIABLES declaration directive "
        "VARIABLES: VARIABLESState",
        f"frontier-fast: {tla}:9 malformed VARIABLES declaration directive "
        "VARIABLE: VARIABLEFlag",
    ]


def test_tla_constant_variable_overlap_errors_rejects_shared_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANTS MaxView, Shared",
                "VARIABLES",
                "  Shared,",
                "  state",
                "vars == <<Shared, state>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_constant_variable_overlap_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:4 declares TLA variable Shared, but line 2 "
        "already declares it as a constant"
    ]


def test_tla_declaration_operator_overlap_errors_rejects_shared_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "CONSTANT Bound",
                "VARIABLES",
                "  state",
                "vars == <<state>>",
                "Bound == TRUE",
                "RECURSIVE state(_)",
                "state(value) == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_declaration_operator_overlap_errors(
        "frontier-fast", tla
    ) == [
        f"frontier-fast: {tla}:6 TLA operator definition Bound overlaps "
        "with constant declaration at line 2",
        f"frontier-fast: {tla}:7 TLA RECURSIVE declaration state overlaps "
        "with variable declaration at line 4",
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
                "  accepted,",
                "  VARIABLESState,",
                "  VARIABLEFlag",
                "vars ==",
                "  <<checked,",
                "    accepted,",
                "    VARIABLESState,",
                "    VARIABLEFlag",
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
                "VARIABLESchecked",
                "VARIABLEhidden",
                "VARIABLES",
                "  checked",
                "  bad-token",
                "VARIABLE, hidden",
                "vars == <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 malformed VARIABLES declaration directive "
        "VARIABLES: VARIABLESchecked",
        f"frontier-fast: {tla}:3 malformed VARIABLES declaration directive "
        "VARIABLE: VARIABLEhidden",
        f"frontier-fast: {tla}:6 VARIABLES declaration line must list static "
        "identifiers: bad-token",
        f"frontier-fast: {tla}:7 VARIABLES declaration line must list static "
        "identifiers: VARIABLE, hidden",
    ]


def test_tla_variable_surface_errors_rejects_indented_directives(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "  VARIABLES hidden",
                "  VARIABLE, hidden",
                "VARIABLES",
                "  checked",
                "vars == <<checked>>",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:2 VARIABLES declaration directive must "
        "be top-level: VARIABLES hidden",
        f"frontier-fast: {tla}:3 VARIABLES declaration directive must "
        "be top-level: VARIABLE, hidden",
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


def test_tla_variable_surface_errors_rejects_malformed_vars_tuple_starts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Model.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Model ----",
                "VARIABLE checked",
                "vars <<checked>>",
                "vars: <<checked>>",
                "vars=<<checked>>",
                "varsTuple == TRUE",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    assert module.tla_variable_surface_errors("frontier-fast", tla) == [
        f"frontier-fast: {tla}:3 malformed vars tuple definition: vars <<checked>>",
        f"frontier-fast: {tla}:4 malformed vars tuple definition: vars: <<checked>>",
        f"frontier-fast: {tla}:5 malformed vars tuple definition: vars=<<checked>>",
        f"frontier-fast: {tla} defines vars tuple 0 times",
        f"frontier-fast: {tla} declares variable checked but vars does not include it",
    ]
    assert module.tla_operator_definitions(tla) == {"varsTuple"}


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


def test_cfg_duplicate_operator_reference_errors_rejects_check_kind_conflicts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "AmbiguousKind.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY TypeInvariant",
                "PROPERTIES Safety EventuallyCommit",
                "INVARIANT Safety",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_operator_reference_errors(
        "frontier-fast", cfg
    ) == [
        f"frontier-fast: {cfg}:4 references PROPERTY check TypeInvariant, "
        "but line 3 already references it as INVARIANT; CFG proof targets "
        "must not be both INVARIANT and PROPERTY",
        f"frontier-fast: {cfg}:6 references INVARIANT check Safety, but line 5 "
        "already references it as PROPERTY; CFG proof targets must not be both "
        "INVARIANT and PROPERTY",
    ]


def test_cfg_duplicate_operator_reference_errors_rejects_role_overlaps(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "RoleOverlap.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT SharedBehavior",
                "NEXT SharedStep",
                "INVARIANT SharedBehavior",
                "CONSTRAINT BoundState",
                "PROPERTY BoundState",
                "NEXT SharedBehavior",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_duplicate_operator_reference_errors(
        "frontier-fast", cfg
    ) == [
        f"frontier-fast: {cfg}:3 references INVARIANT check SharedBehavior, "
        "but line 1 already references it as INIT operator; CFG behavior, "
        "constraint, and proof targets must be role-disjoint",
        f"frontier-fast: {cfg}:5 references PROPERTY check BoundState, but "
        "line 4 already references it as CONSTRAINT operator; CFG behavior, "
        "constraint, and proof targets must be role-disjoint",
        f"frontier-fast: {cfg}:6 references NEXT operator SharedBehavior, but "
        "line 1 already references it as INIT operator; CFG behavior, "
        "constraint, and proof targets must be role-disjoint",
        f"frontier-fast: {cfg}:6 repeats NEXT behavior directive first declared at line 2",
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
        f"frontier-fast: Apalache cfg {cfg} has no model-specific "
        "*CorrectnessEnvelope invariant/property check",
    ]


def test_cfg_fast_generic_check_errors_rejects_fast_checks_without_envelope(
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

    assert module.cfg_fast_generic_check_errors("frontier-fast", cfg, "TLC") == [
        f"frontier-fast: TLC cfg {cfg} has no model-specific "
        "*CorrectnessEnvelope invariant/property check"
    ]


def test_cfg_fast_generic_check_errors_accepts_enveloped_fast_checks(
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
                "INVARIANT FrontierExactness",
                "INVARIANT FrontierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_fast_generic_check_errors("frontier-fast", cfg, "TLC") == []


def test_cfg_fast_generic_check_errors_ignores_mutation_configs(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiFrontier_bug_stale_not_fast.cfg"
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


def test_cfg_correctness_envelope_shape_errors_rejects_missing_type_invariant(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiFrontier.tla"
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiFrontier ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FrontierDecisionMatchesSpec == checked = ready",
                "FrontierExactness ==",
                "  /\\ FrontierDecisionMatchesSpec",
                "FrontierCorrectnessEnvelope ==",
                "  /\\ FrontierExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT FrontierExactness",
                "INVARIANT FrontierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "frontier-fast", tla, cfg, "Apalache"
    ) == [
        f"frontier-fast: Apalache cfg {cfg}:5 references correctness envelope "
        f"FrontierCorrectnessEnvelope, but {tla}:9 does not compose TypeInvariant"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_missing_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiFrontier.tla"
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiFrontier ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SafetyFast == TRUE",
                "FrontierCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyFast",
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
                "INVARIANT TypeInvariant",
                "INVARIANT FrontierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "frontier-fast", tla, cfg, "TLC"
    ) == [
        f"frontier-fast: TLC cfg {cfg}:4 references correctness envelope "
        f"FrontierCorrectnessEnvelope, but {tla}:7 has no model-specific "
        "*Exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_accepts_exactness_envelope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiFrontier.tla"
    cfg = tmp_path / "SumeragiFrontier_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiFrontier ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FrontierDecisionMatchesSpec == checked = ready",
                "FrontierExactness ==",
                "  /\\ FrontierDecisionMatchesSpec",
                "FrontierCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ FrontierExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT FrontierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "frontier-fast", tla, cfg, "Apalache"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_ignore_string_literal_identifiers(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStringIdentifierEnvelope.tla"
    cfg = tmp_path / "SumeragiStringIdentifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStringIdentifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                'ConcreteStringPredicate == label = "TypeInvariant"',
                "StringIdentifierExactness ==",
                "  /\\ ConcreteStringPredicate",
                "StringIdentifierCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StringIdentifierExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StringIdentifierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "string-identifier-envelope-fast", tla, cfg, "TLC"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_has_no_legacy_exactness_debt(
) -> None:
    module = load_coverage_module()
    assert module.LEGACY_FAST_ENVELOPE_WITHOUT_EXACTNESS == set()


def test_cfg_correctness_envelope_shape_errors_has_no_direct_alias_exceptions(
) -> None:
    module = load_coverage_module()
    assert not hasattr(module, "DIRECT_EXACTNESS_ALIAS_EXCEPTIONS")


def test_cfg_correctness_envelope_shape_errors_rejects_fast_missing_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SafetyFast == TRUE",
                "SumeragiConsensusCoreSafetyAnchors == TRUE",
                "SumeragiConsensusCoreFastCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyFast",
                "  /\\ SumeragiConsensusCoreSafetyAnchors",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SumeragiConsensusCoreFastCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        f"SumeragiConsensusCoreFastCorrectnessEnvelope, but {tla}:8 has no "
        "model-specific *Exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLiteral.tla"
    cfg = tmp_path / "SumeragiLiteral_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLiteral ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LiteralExactness == TRUE",
                "LiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "literal-fast", tla, cfg, "Apalache"
    ) == [
        f"literal-fast: Apalache cfg {cfg}:4 references correctness envelope "
        f"LiteralCorrectnessEnvelope, but exactness conjunct LiteralExactness "
        f"at {tla}:5 is literal TRUE"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nobuginvariant_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNoBug.tla"
    cfg = tmp_path / "SumeragiNoBug_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNoBug ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "NoBugInvariant == TRUE",
                "NoBugExactness == NoBugInvariant",
                "NoBugCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NoBugExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NoBugCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nobug-fast", tla, cfg, "TLC"
    ) == [
        f"nobug-fast: TLC cfg {cfg}:4 references correctness envelope "
        f"NoBugCorrectnessEnvelope, but exactness conjunct NoBugExactness "
        f"at {tla}:6 aliases generic NoBugInvariant; compose concrete model "
        "predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_safetyfast_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSafetyFast.tla"
    cfg = tmp_path / "SumeragiSafetyFast_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSafetyFast ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "SafetyFast == ModelPredicate",
                "SafetyFastExactness == SafetyFast",
                "SafetyFastCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyFastExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SafetyFastCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "safetyfast-fast", tla, cfg, "Apalache"
    ) == [
        f"safetyfast-fast: Apalache cfg {cfg}:4 references correctness "
        "envelope SafetyFastCorrectnessEnvelope, but exactness conjunct "
        f"SafetyFastExactness at {tla}:7 aliases generic SafetyFast; compose "
        "concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_safety_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSafety.tla"
    cfg = tmp_path / "SumeragiSafety_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSafety ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "Safety == ModelPredicate",
                "SafetyExactness == Safety",
                "SafetyCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SafetyCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "safety-fast", tla, cfg, "TLC"
    ) == [
        f"safety-fast: TLC cfg {cfg}:4 references correctness envelope "
        "SafetyCorrectnessEnvelope, but exactness conjunct SafetyExactness "
        f"at {tla}:7 aliases generic Safety; compose concrete model "
        "predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_mixed_generic_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiMixedSafety.tla"
    cfg = tmp_path / "SumeragiMixedSafety_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiMixedSafety ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "Safety == ModelPredicate",
                "MixedExactness ==",
                "  /\\ Safety",
                "  /\\ ModelPredicate",
                "MixedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ MixedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT MixedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "mixed-safety-fast", tla, cfg, "Apalache"
    ) == [
        f"mixed-safety-fast: Apalache cfg {cfg}:4 references correctness "
        "envelope MixedCorrectnessEnvelope, but exactness conjunct "
        f"MixedExactness at {tla}:8 mentions generic Safety; compose concrete "
        "model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_exactness_direct_alias(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiAliasEnvelope.tla"
    cfg = tmp_path / "SumeragiAliasEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiAliasEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "AliasExactness == ModelPredicate",
                "AliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ AliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT AliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "alias-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"alias-envelope-fast: Apalache cfg {cfg}:4 references correctness "
        "envelope AliasCorrectnessEnvelope, but exactness conjunct "
        f"AliasExactness at {tla}:6 aliases ModelPredicate; inline concrete "
        "model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_duplicate_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDuplicateEnvelope.tla"
    cfg = tmp_path / "SumeragiDuplicateEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDuplicateEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FirstPredicate == TRUE",
                "SecondPredicate == TRUE",
                "DuplicateExactness ==",
                "  /\\ FirstPredicate",
                "  /\\ SecondPredicate",
                "  /\\ FirstPredicate",
                "DuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "duplicate-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"duplicate-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope DuplicateCorrectnessEnvelope, but exactness "
        f"conjunct DuplicateExactness at {tla}:8 repeats exactness conjunct "
        "FirstPredicate; remove duplicate conjuncts so every obligation is "
        "counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLiteralConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiLiteralConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLiteralConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LiteralConjunct == TRUE",
                "LiteralConjunctExactness ==",
                "  /\\ LiteralConjunct",
                "LiteralConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LiteralConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LiteralConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "literal-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"literal-conjunct-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope LiteralConjunctCorrectnessEnvelope, but "
        f"exactness conjunct LiteralConjunctExactness at {tla}:7 contains "
        f"literal exactness conjunct LiteralConjunct at {tla}:5 is literal "
        "TRUE; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_static_if_literal_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStaticIfLiteralConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiStaticIfLiteralConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStaticIfLiteralConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "StaticIfLiteral == IF TRUE THEN TRUE ELSE FALSE",
                "StaticIfLiteralExactness ==",
                "  /\\ StaticIfLiteral",
                "StaticIfLiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StaticIfLiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StaticIfLiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "static-if-literal-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"static-if-literal-conjunct-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "StaticIfLiteralCorrectnessEnvelope, but exactness conjunct "
        f"StaticIfLiteralExactness at {tla}:7 contains literal exactness "
        f"conjunct StaticIfLiteral at {tla}:5 is static IF literal TRUE; "
        "compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_constant_relation_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiConstantRelationConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiConstantRelationConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiConstantRelationConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConstantRelation == TRUE = TRUE",
                "ConstantRelationExactness ==",
                "  /\\ ConstantRelation",
                "ConstantRelationCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ConstantRelationExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ConstantRelationCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "constant-relation-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"constant-relation-conjunct-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "ConstantRelationCorrectnessEnvelope, but exactness conjunct "
        f"ConstantRelationExactness at {tla}:7 contains constant-relation "
        f"exactness conjunct ConstantRelation at {tla}:5 is constant "
        "relation TRUE = TRUE; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_constant_relation_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnaryTemporalConstantRelationConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiUnaryTemporalConstantRelationConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnaryTemporalConstantRelationConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConstantRelation == [] (TRUE = TRUE)",
                "ConstantRelationExactness ==",
                "  /\\ ConstantRelation",
                "ConstantRelationCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ConstantRelationExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ConstantRelationCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unary-temporal-constant-relation-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"unary-temporal-constant-relation-conjunct-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "ConstantRelationCorrectnessEnvelope, but exactness conjunct "
        f"ConstantRelationExactness at {tla}:7 contains constant-relation "
        f"exactness conjunct ConstantRelation at {tla}:5 is constant "
        "relation [] (TRUE = TRUE); compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_temporal_boolean_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStaticBooleanConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiStaticBooleanConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStaticBooleanConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "WrappedLiteral == [] TRUE",
                "WrappedLiteralExactness ==",
                "  /\\ WrappedLiteral",
                "WrappedLiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ WrappedLiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT WrappedLiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "static-boolean-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"static-boolean-conjunct-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope WrappedLiteralCorrectnessEnvelope, "
        f"but exactness conjunct WrappedLiteralExactness at {tla}:7 contains "
        f"literal exactness conjunct WrappedLiteral at {tla}:5 is literal "
        "TRUE; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_negated_temporal_boolean_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNegatedStaticBooleanConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiNegatedStaticBooleanConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNegatedStaticBooleanConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "NegatedWrappedLiteral == ~([] FALSE)",
                "NegatedWrappedLiteralExactness ==",
                "  /\\ NegatedWrappedLiteral",
                "NegatedWrappedLiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NegatedWrappedLiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NegatedWrappedLiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "negated-static-boolean-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"negated-static-boolean-conjunct-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "NegatedWrappedLiteralCorrectnessEnvelope, but exactness conjunct "
        f"NegatedWrappedLiteralExactness at {tla}:7 contains literal "
        f"exactness conjunct NegatedWrappedLiteral at {tla}:5 is literal "
        "TRUE; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_temporal_boolean_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasLiteralEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasLiteralEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasLiteralEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "WrappedLetLiteral == [] (LET selected == TRUE IN selected)",
                "LetAliasLiteralExactness ==",
                "  /\\ WrappedLetLiteral",
                "LetAliasLiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasLiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasLiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-literal-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-literal-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope LetAliasLiteralCorrectnessEnvelope, but "
        f"exactness conjunct LetAliasLiteralExactness at {tla}:7 contains "
        f"literal exactness conjunct WrappedLetLiteral at {tla}:5 is literal "
        "TRUE; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_temporal_boolean_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiCompoundStaticBooleanConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiCompoundStaticBooleanConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiCompoundStaticBooleanConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "CompoundWrappedLiteral == [] (TRUE => TRUE)",
                "CompoundWrappedLiteralExactness ==",
                "  /\\ CompoundWrappedLiteral",
                "CompoundWrappedLiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ CompoundWrappedLiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT CompoundWrappedLiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "compound-static-boolean-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"compound-static-boolean-conjunct-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "CompoundWrappedLiteralCorrectnessEnvelope, but exactness conjunct "
        f"CompoundWrappedLiteralExactness at {tla}:7 contains literal "
        f"exactness conjunct CompoundWrappedLiteral at {tla}:5 is literal "
        "TRUE; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiParameterizedConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ParameterizedPredicate(c) == checked = ready",
                "ParameterizedExactness ==",
                "  /\\ ParameterizedPredicate",
                "ParameterizedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParameterizedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParameterizedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "parameterized-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"parameterized-conjunct-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope ParameterizedCorrectnessEnvelope, but exactness "
        f"conjunct ParameterizedExactness at {tla}:7 contains non-zero-arity "
        f"exactness conjunct ParameterizedPredicate at {tla}:5 has arity 1; "
        "exactness conjuncts must compose zero-arity model predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_parameterized_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveParameterizedConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveParameterizedConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveParameterizedConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "ParameterizedLeaf(c) == ModelPredicate",
                "ParameterizedWrapper ==",
                "  /\\ ParameterizedLeaf",
                "TransitiveParameterizedExactness ==",
                "  /\\ ParameterizedWrapper",
                "TransitiveParameterizedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveParameterizedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveParameterizedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-parameterized-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-parameterized-conjunct-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveParameterizedCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveParameterizedExactness at {tla}:10 contains transitive "
        "exactness predicate chain with non-zero-arity conjunct "
        f"ParameterizedWrapper reaches ParameterizedWrapper -> "
        f"ParameterizedLeaf at {tla}:6 with arity 1; exactness predicate chains "
        "must compose zero-arity model predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_transitive_parameterized_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiCompoundTransitiveParameterizedConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiCompoundTransitiveParameterizedConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiCompoundTransitiveParameterizedConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "ParameterizedLeaf(c) == ModelPredicate",
                "ParameterizedWrapper == ParameterizedLeaf \\/ ModelPredicate",
                "CompoundTransitiveParameterizedExactness ==",
                "  /\\ ParameterizedWrapper",
                "CompoundTransitiveParameterizedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ CompoundTransitiveParameterizedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT CompoundTransitiveParameterizedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "compound-transitive-parameterized-conjunct-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "compound-transitive-parameterized-conjunct-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "CompoundTransitiveParameterizedCorrectnessEnvelope, but exactness "
        f"conjunct CompoundTransitiveParameterizedExactness at {tla}:9 "
        "contains transitive exactness predicate chain with non-zero-arity "
        "conjunct ParameterizedWrapper reaches ParameterizedWrapper -> "
        f"ParameterizedLeaf at {tla}:6 with arity 1; exactness predicate "
        "chains must compose zero-arity model predicates"
    ]


def test_unary_temporal_parameterized_calls_unwrap_let_aliases() -> None:
    module = load_coverage_module()

    assert module.unary_temporal_parameterized_calls("[] Predicate(Cases)") == [
        "Predicate(Cases)"
    ]
    assert module.unary_temporal_parameterized_calls(
        "[] Predicate(ready = checked)"
    ) == ["Predicate(ready = checked)"]
    assert module.unary_temporal_parameterized_calls(
        "[] (LET selected == Predicate(Cases) IN selected)"
    ) == ["Predicate(Cases)"]
    assert module.unary_temporal_parameterized_calls(
        "LET selected == Predicate(Cases) IN [] selected"
    ) == ["Predicate(Cases)"]
    assert module.unary_temporal_parameterized_calls(
        "ConcretePredicate /\\ (LET selected == Predicate(Cases) IN [] selected)"
    ) == ["Predicate(Cases)"]
    assert module.unary_temporal_parameterized_calls(
        "[] Predicate(Cases) /\\ OtherPredicate(Cases)"
    ) == ["Predicate(Cases)"]
    assert module.unary_temporal_parameterized_calls(
        "[](Predicate(Cases) /\\ OtherPredicate(Cases))"
    ) == ["Predicate(Cases)", "OtherPredicate(Cases)"]
    assert module.unary_temporal_parameterized_calls("Predicate(Cases)") == []


def test_compound_parameterized_helper_calls_preserve_leaf_calls() -> None:
    module = load_coverage_module()
    signatures = {"Predicate": (1, 1)}

    assert module.compound_parameterized_helper_calls(
        "Predicate(Cases)",
        signatures,
    ) == []
    assert module.compound_parameterized_helper_calls(
        "LET selected == Predicate(Cases) IN selected",
        signatures,
    ) == []
    assert module.compound_parameterized_helper_calls(
        "ConcretePredicate /\\ Predicate(Cases)",
        signatures,
    ) == []
    assert module.compound_parameterized_helper_calls(
        "ConcretePredicate /\\ Predicate(TRUE)",
        signatures,
    ) == []
    assert module.compound_parameterized_helper_calls(
        "TRUE /\\ Predicate(ready = checked)",
        signatures,
    ) == ["Predicate(ready = checked)"]
    assert module.compound_parameterized_helper_calls(
        "ConcretePredicate /\\ Predicate(ready = checked)",
        signatures,
    ) == ["Predicate(ready = checked)"]
    assert module.compound_parameterized_helper_calls(
        "FALSE \\/ Predicate(ready = checked)",
        signatures,
    ) == ["Predicate(ready = checked)"]
    assert module.compound_parameterized_helper_calls(
        "TRUE => Predicate(ready = checked)",
        signatures,
    ) == ["Predicate(ready = checked)"]
    assert module.compound_parameterized_helper_calls(
        "TRUE <=> Predicate(ready = checked)",
        signatures,
    ) == ["Predicate(ready = checked)"]
    assert module.compound_parameterized_helper_calls(
        "[] Predicate(ready = checked)",
        signatures,
    ) == ["Predicate(ready = checked)"]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_parameterized_exactness_call(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveParameterizedCallEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveParameterizedCallEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveParameterizedCallEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ParameterizedLeaf(c) == checked = ready",
                "ParameterizedCallWrapper == [] ParameterizedLeaf(1)",
                "TransitiveParameterizedCallExactness ==",
                "  /\\ ParameterizedCallWrapper",
                "TransitiveParameterizedCallCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveParameterizedCallExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveParameterizedCallCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-parameterized-call-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "transitive-parameterized-call-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "TransitiveParameterizedCallCorrectnessEnvelope, but exactness "
        f"conjunct TransitiveParameterizedCallExactness at {tla}:8 contains "
        "transitive exactness predicate chain with parameterized helper call "
        "ParameterizedCallWrapper reaches ParameterizedLeaf(1) through "
        f"ParameterizedCallWrapper at {tla}:6; lift exactness helper calls "
        "behind zero-arity model predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_parameterized_exactness_call_with_expression_argument(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveParameterizedCallArgumentEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveParameterizedCallArgumentEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveParameterizedCallArgumentEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ParameterizedLeaf(c) == c = c",
                "ParameterizedCallWrapper == [] ParameterizedLeaf(ready = checked)",
                "TransitiveParameterizedCallExactness ==",
                "  /\\ ParameterizedCallWrapper",
                "TransitiveParameterizedCallCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveParameterizedCallExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveParameterizedCallCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-parameterized-call-argument-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "transitive-parameterized-call-argument-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "TransitiveParameterizedCallCorrectnessEnvelope, but exactness "
        f"conjunct TransitiveParameterizedCallExactness at {tla}:8 contains "
        "transitive exactness predicate chain with parameterized helper call "
        "ParameterizedCallWrapper reaches ParameterizedLeaf(ready = checked) "
        f"through ParameterizedCallWrapper at {tla}:6; lift exactness helper "
        "calls behind zero-arity model predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_transitive_parameterized_exactness_call_operands(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cases = (
        ("literal-conjunction", "TRUE /\\ ParameterizedPredicate(ready = checked)"),
        (
            "predicate-conjunction",
            "ConcretePredicate /\\ ParameterizedPredicate(ready = checked)",
        ),
        ("disjunction", "FALSE \\/ ParameterizedPredicate(ready = checked)"),
        ("implication", "TRUE => ParameterizedPredicate(ready = checked)"),
        ("equivalence", "TRUE <=> ParameterizedPredicate(ready = checked)"),
    )
    for case_name, wrapper_body in cases:
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "SumeragiCompoundParameterizedCallEnvelope.tla"
        cfg = case_dir / "SumeragiCompoundParameterizedCallEnvelope_fast.cfg"
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE SumeragiCompoundParameterizedCallEnvelope ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "ConcretePredicate == TRUE",
                    "ParameterizedPredicate(c) == c = c",
                    f"Wrapper == {wrapper_body}",
                    "Exactness ==",
                    "  /\\ Wrapper",
                    "CorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ Exactness",
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
                    "INVARIANT TypeInvariant",
                    "INVARIANT CorrectnessEnvelope",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "compound-parameterized-call-envelope-fast",
            tla,
            cfg,
            "TLC",
        ) == [
            "compound-parameterized-call-envelope-fast: TLC cfg "
            f"{cfg}:4 references correctness envelope CorrectnessEnvelope, "
            f"but exactness conjunct Exactness at {tla}:9 contains "
            "transitive exactness predicate chain with parameterized helper "
            "call Wrapper reaches ParameterizedPredicate(ready = checked) "
            f"through Wrapper at {tla}:7; lift exactness helper calls behind "
            "zero-arity model predicates"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_parameterized_exactness_call(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasParameterizedCallEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasParameterizedCallEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasParameterizedCallEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ParameterizedLeaf(c) == c = c",
                "ParameterizedCallWrapper == [] (LET selected == ParameterizedLeaf(1) IN selected)",
                "LetAliasParameterizedCallExactness ==",
                "  /\\ ParameterizedCallWrapper",
                "LetAliasParameterizedCallCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasParameterizedCallExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasParameterizedCallCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-parameterized-call-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "let-alias-parameterized-call-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "LetAliasParameterizedCallCorrectnessEnvelope, but exactness conjunct "
        f"LetAliasParameterizedCallExactness at {tla}:8 contains transitive "
        "exactness predicate chain with parameterized helper call "
        "ParameterizedCallWrapper reaches ParameterizedLeaf(1) through "
        f"ParameterizedCallWrapper at {tla}:6; lift exactness helper calls "
        "behind zero-arity model predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_hidden_coverage_conjuncts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiHiddenCoverageConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiHiddenCoverageConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiHiddenCoverageConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SafetyFast == ModelPredicate",
                "ModelPredicate == checked = ready",
                "OtherExactness ==",
                "  /\\ ModelPredicate",
                "HiddenCoverageAnchors ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyFast",
                "  /\\ OtherExactness",
                "  /\\ ModelPredicate",
                "HiddenCoverageExactness ==",
                "  /\\ HiddenCoverageAnchors",
                "HiddenCoverageCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ HiddenCoverageExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT HiddenCoverageCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "hidden-coverage-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"hidden-coverage-conjunct-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope HiddenCoverageCorrectnessEnvelope, but "
        f"exactness conjunct HiddenCoverageExactness at {tla}:15 contains "
        "exactness conjunct with hidden coverage identifiers "
        f"HiddenCoverageAnchors at {tla}:10 mentions OtherExactness, "
        "SafetyFast, TypeInvariant; keep TypeInvariant, generic correctness, "
        "and nested *Exactness identifiers out of named exactness predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_hidden_coverage(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveHiddenCoverageEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveHiddenCoverageEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveHiddenCoverageEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SafetyFast == ModelPredicate",
                "ModelPredicate == checked = ready",
                "OtherExactness ==",
                "  /\\ ModelPredicate",
                "HiddenCoverageLeaf ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyFast",
                "  /\\ OtherExactness",
                "  /\\ ModelPredicate",
                "HiddenCoverageWrapper ==",
                "  /\\ HiddenCoverageLeaf",
                "TransitiveHiddenCoverageExactness ==",
                "  /\\ HiddenCoverageWrapper",
                "TransitiveHiddenCoverageCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveHiddenCoverageExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveHiddenCoverageCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-hidden-coverage-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-hidden-coverage-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveHiddenCoverageCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveHiddenCoverageExactness at {tla}:17 contains transitive "
        "exactness predicate chain with hidden coverage identifiers "
        "HiddenCoverageWrapper reaches OtherExactness through "
        f"HiddenCoverageWrapper -> HiddenCoverageLeaf at {tla}:10, "
        "HiddenCoverageWrapper reaches SafetyFast through "
        f"HiddenCoverageWrapper -> HiddenCoverageLeaf at {tla}:10, "
        "HiddenCoverageWrapper reaches TypeInvariant through "
        f"HiddenCoverageWrapper -> HiddenCoverageLeaf at {tla}:10; keep "
        "TypeInvariant, generic correctness, and nested *Exactness identifiers "
        "out of named exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_duplicate_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveDuplicateExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveDuplicateExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveDuplicateExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked = ready",
                "DuplicateWrapper ==",
                "  /\\ LeafPredicate",
                "  /\\ LeafPredicate",
                "TransitiveDuplicateExactness ==",
                "  /\\ DuplicateWrapper",
                "TransitiveDuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveDuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveDuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-duplicate-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"transitive-duplicate-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveDuplicateCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveDuplicateExactness at {tla}:10 contains transitive "
        "exactness predicate chain with repeated helper conjunct "
        "DuplicateWrapper reaches DuplicateWrapper through DuplicateWrapper "
        f"at {tla}:7 repeats LeafPredicate; remove duplicate helper "
        "conjuncts so every obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_wrapped_duplicate_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiWrappedDuplicateExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiWrappedDuplicateExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiWrappedDuplicateExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked = ready",
                "DuplicateWrapper == [] (LeafPredicate /\\ [] LeafPredicate)",
                "WrappedDuplicateExactness ==",
                "  /\\ DuplicateWrapper",
                "WrappedDuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ WrappedDuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT WrappedDuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "wrapped-duplicate-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"wrapped-duplicate-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "WrappedDuplicateCorrectnessEnvelope, but exactness conjunct "
        f"WrappedDuplicateExactness at {tla}:8 contains transitive "
        "exactness predicate chain with repeated helper conjunct "
        "DuplicateWrapper reaches DuplicateWrapper through DuplicateWrapper "
        f"at {tla}:6 repeats LeafPredicate; remove duplicate helper "
        "conjuncts so every obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_duplicate_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasDuplicateExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasDuplicateExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasDuplicateExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked = ready",
                "DuplicateWrapper == LeafPredicate /\\ (LET selected == LeafPredicate IN selected)",
                "LetAliasDuplicateExactness ==",
                "  /\\ DuplicateWrapper",
                "LetAliasDuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasDuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasDuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-duplicate-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-duplicate-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope LetAliasDuplicateCorrectnessEnvelope, "
        f"but exactness conjunct LetAliasDuplicateExactness at {tla}:8 "
        "contains transitive exactness predicate chain with repeated helper "
        "conjunct DuplicateWrapper reaches DuplicateWrapper through "
        f"DuplicateWrapper at {tla}:6 repeats LeafPredicate; remove duplicate "
        "helper conjuncts so every obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_duplicate_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiCompoundDuplicateExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiCompoundDuplicateExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiCompoundDuplicateExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked = ready",
                "OtherPredicate == 2 = 2",
                "DuplicateWrapper == (LeafPredicate /\\ LeafPredicate) \\/ OtherPredicate",
                "CompoundDuplicateExactness ==",
                "  /\\ DuplicateWrapper",
                "CompoundDuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ CompoundDuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT CompoundDuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "compound-duplicate-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"compound-duplicate-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "CompoundDuplicateCorrectnessEnvelope, but exactness conjunct "
        f"CompoundDuplicateExactness at {tla}:9 contains transitive "
        "exactness predicate chain with repeated helper conjunct "
        "DuplicateWrapper reaches DuplicateWrapper through DuplicateWrapper "
        f"at {tla}:7 repeats LeafPredicate; remove duplicate helper "
        "conjuncts so every obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_repeated_boolean_operand_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRepeatedOperandExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiRepeatedOperandExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRepeatedOperandExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked /\\ ready",
                "RepeatedOperandWrapper == LeafPredicate \\/ [] LeafPredicate",
                "RepeatedOperandExactness ==",
                "  /\\ RepeatedOperandWrapper",
                "RepeatedOperandCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ RepeatedOperandExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT RepeatedOperandCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "repeated-operand-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"repeated-operand-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope RepeatedOperandCorrectnessEnvelope, "
        f"but exactness conjunct RepeatedOperandExactness at {tla}:8 "
        "contains transitive exactness predicate chain with repeated helper "
        "operand RepeatedOperandWrapper reaches RepeatedOperandWrapper "
        f"through RepeatedOperandWrapper at {tla}:6 repeats LeafPredicate; "
        "remove duplicate helper operands so every obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_contradictory_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiContradictoryExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiContradictoryExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiContradictoryExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked /\\ ready",
                "ContradictoryWrapper == LeafPredicate /\\ ~LeafPredicate",
                "ContradictoryExactness ==",
                "  /\\ ContradictoryWrapper",
                "ContradictoryCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ContradictoryExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ContradictoryCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "contradictory-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"contradictory-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope ContradictoryCorrectnessEnvelope, "
        f"but exactness conjunct ContradictoryExactness at {tla}:8 contains "
        "transitive exactness predicate chain with contradictory helper "
        "operand ContradictoryWrapper reaches ContradictoryWrapper through "
        f"ContradictoryWrapper at {tla}:6 pairs LeafPredicate with "
        "~LeafPredicate; name concrete non-contradictory model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_excluded_middle_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiExcludedMiddleExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiExcludedMiddleExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiExcludedMiddleExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked /\\ ready",
                "ExcludedMiddleWrapper == LeafPredicate \\/ ~LeafPredicate",
                "ExcludedMiddleExactness ==",
                "  /\\ ExcludedMiddleWrapper",
                "ExcludedMiddleCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ExcludedMiddleExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ExcludedMiddleCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "excluded-middle-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"excluded-middle-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope ExcludedMiddleCorrectnessEnvelope, "
        f"but exactness conjunct ExcludedMiddleExactness at {tla}:8 contains "
        "transitive exactness predicate chain with excluded-middle helper "
        "operand ExcludedMiddleWrapper reaches ExcludedMiddleWrapper through "
        f"ExcludedMiddleWrapper at {tla}:6 pairs LeafPredicate with "
        "~LeafPredicate; name concrete non-tautological model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_complementary_equivalence_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiComplementaryEquivalenceExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiComplementaryEquivalenceExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiComplementaryEquivalenceExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeafPredicate == checked /\\ ready",
                "ComplementaryEquivalenceWrapper == LeafPredicate <=> ~LeafPredicate",
                "ComplementaryEquivalenceExactness ==",
                "  /\\ ComplementaryEquivalenceWrapper",
                "ComplementaryEquivalenceCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ComplementaryEquivalenceExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ComplementaryEquivalenceCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "complementary-equivalence-exactness-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"complementary-equivalence-exactness-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "ComplementaryEquivalenceCorrectnessEnvelope, but exactness conjunct "
        f"ComplementaryEquivalenceExactness at {tla}:8 contains transitive "
        "exactness predicate chain with complementary-equivalence helper "
        "operand ComplementaryEquivalenceWrapper reaches "
        "ComplementaryEquivalenceWrapper through "
        f"ComplementaryEquivalenceWrapper at {tla}:6 pairs LeafPredicate with "
        "~LeafPredicate under equivalence; name concrete non-vacuous model "
        "predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_repeated_state_terms_in_concrete_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRepeatedStateTermsExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiRepeatedStateTermsExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRepeatedStateTermsExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate ==",
                "  /\\ checked",
                "  /\\ checked",
                "RepeatedStateTermsExactness ==",
                "  /\\ ConcretePredicate",
                "RepeatedStateTermsCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ RepeatedStateTermsExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT RepeatedStateTermsCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "repeated-state-terms-exactness-envelope-fast", tla, cfg, "Apalache"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "OtherPredicate == 2 = 2",
                "ControlFlowWrapper == IF TRUE THEN ModelPredicate ELSE OtherPredicate",
                "TransitiveControlFlowExactness ==",
                "  /\\ ControlFlowWrapper",
                "TransitiveControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-control-flow-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-control-flow-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveControlFlowExactness at {tla}:9 contains transitive "
        "exactness predicate chain with whole-body control-flow "
        "predicate-selection helper "
        "ControlFlowWrapper reaches ControlFlowWrapper through "
        f"ControlFlowWrapper at {tla}:7 is whole-body IF expression IF TRUE "
        "THEN ModelPredicate ELSE OtherPredicate; name concrete model "
        "predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnaryTemporalControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiUnaryTemporalControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnaryTemporalControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "OtherPredicate == 2 = 2",
                "ControlFlowWrapper == [] (IF TRUE THEN ModelPredicate ELSE OtherPredicate)",
                "UnaryTemporalControlFlowExactness ==",
                "  /\\ ControlFlowWrapper",
                "UnaryTemporalControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ UnaryTemporalControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnaryTemporalControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unary-temporal-control-flow-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"unary-temporal-control-flow-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "UnaryTemporalControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"UnaryTemporalControlFlowExactness at {tla}:9 contains transitive "
        "exactness predicate chain with unary-temporal control-flow "
        "predicate-selection helper ControlFlowWrapper reaches "
        "ControlFlowWrapper through ControlFlowWrapper at "
        f"{tla}:7 is unary-temporal IF expression IF TRUE THEN "
        "ModelPredicate ELSE OtherPredicate; name concrete model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_let_alias_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnaryTemporalLetAliasEnvelope.tla"
    cfg = tmp_path / "SumeragiUnaryTemporalLetAliasEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnaryTemporalLetAliasEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "UnaryTemporalLetAliasWrapper == [] (LET selected == ConcretePredicate IN selected)",
                "UnaryTemporalLetAliasExactness ==",
                "  /\\ UnaryTemporalLetAliasWrapper",
                "UnaryTemporalLetAliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ UnaryTemporalLetAliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnaryTemporalLetAliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unary-temporal-let-alias-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"unary-temporal-let-alias-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "UnaryTemporalLetAliasCorrectnessEnvelope, but exactness conjunct "
        f"UnaryTemporalLetAliasExactness at {tla}:8 contains transitive "
        "exactness predicate chain with unary-temporal LET alias "
        "UnaryTemporalLetAliasWrapper reaches UnaryTemporalLetAliasWrapper "
        f"through UnaryTemporalLetAliasWrapper at {tla}:6 contains [] (LET "
        "selected == ConcretePredicate IN selected) aliases ConcretePredicate; "
        "name concrete model predicates before composing exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "RightPredicate == ready = checked",
                "NestedControlFlowWrapper == ConcretePredicate /\\ (IF ready THEN LeftPredicate ELSE RightPredicate)",
                "NestedControlFlowExactness ==",
                "  /\\ NestedControlFlowWrapper",
                "NestedControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NestedControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NestedControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"nested-control-flow-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope NestedControlFlowCorrectnessEnvelope, but "
        f"exactness conjunct NestedControlFlowExactness at {tla}:10 contains "
        "transitive exactness predicate chain with nested control-flow "
        "predicate-selection helper NestedControlFlowWrapper reaches "
        "NestedControlFlowWrapper through NestedControlFlowWrapper at "
        f"{tla}:8 contains nested IF expression IF ready THEN LeftPredicate "
        "ELSE RightPredicate; name concrete model predicates before composing "
        "exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_enabled_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedEnabledControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedEnabledControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedEnabledControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "NestedEnabledControlFlowWrapper == ConcretePredicate /\\ (ENABLED LeftPredicate)",
                "NestedEnabledControlFlowExactness ==",
                "  /\\ NestedEnabledControlFlowWrapper",
                "NestedEnabledControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NestedEnabledControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NestedEnabledControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-enabled-control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"nested-enabled-control-flow-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "NestedEnabledControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"NestedEnabledControlFlowExactness at {tla}:9 contains transitive "
        "exactness predicate chain with nested control-flow "
        "predicate-selection helper NestedEnabledControlFlowWrapper reaches "
        "NestedEnabledControlFlowWrapper through "
        f"NestedEnabledControlFlowWrapper at {tla}:7 contains nested ENABLED "
        "expression ENABLED LeftPredicate; name concrete model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "RightPredicate == ready = checked",
                "LetAliasControlFlowWrapper == ConcretePredicate /\\ "
                "(IF ready THEN LET selected == LeftPredicate IN selected "
                "ELSE LET fallback == RightPredicate IN fallback)",
                "LetAliasControlFlowExactness ==",
                "  /\\ LetAliasControlFlowWrapper",
                "LetAliasControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-control-flow-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope LetAliasControlFlowCorrectnessEnvelope, but "
        f"exactness conjunct LetAliasControlFlowExactness at {tla}:10 "
        "contains transitive exactness predicate chain with nested control-flow "
        "predicate-selection helper LetAliasControlFlowWrapper reaches "
        "LetAliasControlFlowWrapper through LetAliasControlFlowWrapper at "
        f"{tla}:8 contains nested IF expression IF ready THEN LET selected == "
        "LeftPredicate IN selected ELSE LET fallback == RightPredicate IN "
        "fallback; name concrete model predicates before composing exactness "
        "predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_nested_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasNestedControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasNestedControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasNestedControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "RightPredicate == ready = checked",
                "LetAliasNestedControlFlowWrapper == ConcretePredicate /\\ (LET selected == IF ready THEN LeftPredicate ELSE RightPredicate IN selected)",
                "LetAliasNestedControlFlowExactness ==",
                "  /\\ LetAliasNestedControlFlowWrapper",
                "LetAliasNestedControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasNestedControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasNestedControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-nested-control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-nested-control-flow-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "LetAliasNestedControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"LetAliasNestedControlFlowExactness at {tla}:10 contains transitive "
        "exactness predicate chain with nested control-flow "
        "predicate-selection helper LetAliasNestedControlFlowWrapper reaches "
        "LetAliasNestedControlFlowWrapper through "
        f"LetAliasNestedControlFlowWrapper at {tla}:8 contains nested IF "
        "expression IF ready THEN LeftPredicate ELSE RightPredicate; name "
        "concrete model predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_result_wrapper_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasResultWrapperControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasResultWrapperControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasResultWrapperControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "RightPredicate == ready = checked",
                "LetAliasResultWrapperControlFlowWrapper == ConcretePredicate /\\ "
                "(IF ready THEN LET selected == LeftPredicate IN [] selected "
                "ELSE LET fallback == RightPredicate IN <> fallback)",
                "LetAliasResultWrapperControlFlowExactness ==",
                "  /\\ LetAliasResultWrapperControlFlowWrapper",
                "LetAliasResultWrapperControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasResultWrapperControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasResultWrapperControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-result-wrapper-control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-result-wrapper-control-flow-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "LetAliasResultWrapperControlFlowCorrectnessEnvelope, but exactness "
        f"conjunct LetAliasResultWrapperControlFlowExactness at {tla}:10 "
        "contains transitive exactness predicate chain with nested control-flow "
        "predicate-selection helper LetAliasResultWrapperControlFlowWrapper "
        "reaches LetAliasResultWrapperControlFlowWrapper through "
        f"LetAliasResultWrapperControlFlowWrapper at {tla}:8 contains nested "
        "IF expression IF ready THEN LET selected == LeftPredicate IN [] "
        "selected ELSE LET fallback == RightPredicate IN <> fallback; name "
        "concrete model predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_reference_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasReferenceEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasReferenceEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasReferenceEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "HiddenLiteral == TRUE",
                "LetAliasReferenceWrapper == ConcretePredicate /\\ "
                "(LET selected == HiddenLiteral IN selected)",
                "LetAliasReferenceExactness ==",
                "  /\\ LetAliasReferenceWrapper",
                "LetAliasReferenceCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasReferenceExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasReferenceCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-reference-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-reference-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope LetAliasReferenceCorrectnessEnvelope, but "
        f"exactness conjunct LetAliasReferenceExactness at {tla}:9 contains "
        "transitive exactness predicate chain with vacuous conjunct "
        "LetAliasReferenceWrapper reaches HiddenLiteral through "
        "LetAliasReferenceWrapper -> HiddenLiteral at "
        f"{tla}:6 is literal TRUE; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_transitive_bug_selector_boolean_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiBugSelectorBooleanEnvelope.tla"
    cfg = tmp_path / "SumeragiBugSelectorBooleanEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiBugSelectorBooleanEnvelope ----",
                "CONSTANT Bug",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "BugValue == 1",
                "BugSelectorLeaf == IF Bug = BugValue THEN TRUE ELSE FALSE",
                "BugSelectorWrapper ==",
                "  /\\ ~BugSelectorLeaf",
                "BugSelectorBooleanExactness ==",
                "  /\\ BugSelectorWrapper",
                "BugSelectorBooleanCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ BugSelectorBooleanExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    cfg.write_text(
        "\n".join(
            [
                "CONSTANT Bug = 0",
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "INVARIANT BugSelectorBooleanCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "bug-selector-boolean-envelope-fast", tla, cfg, "TLC"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_boolean_composition_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cases = (
        ("negation", "negation", "~LeftPredicate"),
        ("disjunction", "disjunction", "LeftPredicate \\/ RightPredicate"),
        ("implication", "implication", "LeftPredicate => RightPredicate"),
        ("negated-implication", "implication", "LeftPredicate => ~RightPredicate"),
        (
            "stacked-negated-implication",
            "implication",
            "LeftPredicate => ~~RightPredicate",
        ),
        (
            "unary-temporal-implication",
            "implication",
            "LeftPredicate => [] RightPredicate",
        ),
        ("literal-gated-negation", "negation", "TRUE /\\ ~LeftPredicate"),
        ("equivalence", "equivalence", "LeftPredicate <=> RightPredicate"),
    )
    for case_name, operator_name, wrapper_body in cases:
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "SumeragiTransitiveBooleanCompositionEnvelope.tla"
        cfg = case_dir / "SumeragiTransitiveBooleanCompositionEnvelope_fast.cfg"
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE SumeragiTransitiveBooleanCompositionEnvelope ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "LeftPredicate == checked = ready",
                    "RightPredicate == ready = checked",
                    f"BooleanCompositionWrapper == {wrapper_body}",
                    "TransitiveBooleanCompositionExactness ==",
                    "  /\\ BooleanCompositionWrapper",
                    "TransitiveBooleanCompositionCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ TransitiveBooleanCompositionExactness",
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
                    "INVARIANT TypeInvariant",
                    "INVARIANT TransitiveBooleanCompositionCorrectnessEnvelope",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "transitive-boolean-composition-envelope-fast", tla, cfg, "TLC"
        ) == [
            f"transitive-boolean-composition-envelope-fast: TLC cfg {cfg}:4 "
            "references correctness envelope "
            "TransitiveBooleanCompositionCorrectnessEnvelope, but exactness "
            f"conjunct TransitiveBooleanCompositionExactness at {tla}:9 "
            "contains transitive exactness predicate chain with whole-body "
            "raw-predicate boolean-composition helper "
            "BooleanCompositionWrapper reaches BooleanCompositionWrapper "
            f"through BooleanCompositionWrapper at {tla}:7 is whole-body "
            f"{operator_name} {wrapper_body}; name concrete model predicates "
            "before composing exactness predicate chains"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_boolean_composition_exactness_operand(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedBooleanCompositionEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedBooleanCompositionEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedBooleanCompositionEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "RightPredicate == ready = checked",
                "NestedBooleanCompositionWrapper == ConcretePredicate /\\ (LeftPredicate \\/ RightPredicate)",
                "NestedBooleanCompositionExactness ==",
                "  /\\ NestedBooleanCompositionWrapper",
                "NestedBooleanCompositionCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NestedBooleanCompositionExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NestedBooleanCompositionCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-boolean-composition-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"nested-boolean-composition-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "NestedBooleanCompositionCorrectnessEnvelope, but exactness conjunct "
        f"NestedBooleanCompositionExactness at {tla}:10 contains transitive "
        "exactness predicate chain with whole-body raw-predicate "
        "boolean-composition helper NestedBooleanCompositionWrapper reaches "
        "NestedBooleanCompositionWrapper through "
        f"NestedBooleanCompositionWrapper at {tla}:8 is whole-body "
        "disjunction ConcretePredicate /\\ (LeftPredicate \\/ RightPredicate); "
        "name concrete model predicates before composing exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_boolean_composition_exactness_operand(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasBooleanCompositionEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasBooleanCompositionEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasBooleanCompositionEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = prepared",
                "LeftPredicate == checked = ready",
                "RightPredicate == ready = checked",
                "LetAliasBooleanCompositionWrapper == ConcretePredicate /\\ "
                "(LET selected == LeftPredicate \\/ RightPredicate IN selected)",
                "LetAliasBooleanCompositionExactness ==",
                "  /\\ LetAliasBooleanCompositionWrapper",
                "LetAliasBooleanCompositionCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasBooleanCompositionExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasBooleanCompositionCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-boolean-composition-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-boolean-composition-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "LetAliasBooleanCompositionCorrectnessEnvelope, but exactness conjunct "
        f"LetAliasBooleanCompositionExactness at {tla}:10 contains transitive "
        "exactness predicate chain with whole-body raw-predicate "
        "boolean-composition helper LetAliasBooleanCompositionWrapper reaches "
        "LetAliasBooleanCompositionWrapper through "
        f"LetAliasBooleanCompositionWrapper at {tla}:8 is whole-body "
        "disjunction ConcretePredicate /\\ (LET selected == LeftPredicate \\/ "
        "RightPredicate IN selected); name concrete model predicates before "
        "composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_transitive_concrete_formula_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiConcreteFormulaExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiConcreteFormulaExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiConcreteFormulaExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteFormulaLeaf == checked = ready",
                "ConcreteFormulaExactness ==",
                "  /\\ ConcreteFormulaLeaf",
                "ConcreteFormulaCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ConcreteFormulaExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ConcreteFormulaCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "concrete-formula-envelope-fast", tla, cfg, "Apalache"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_parameterized_call_boolean_composition_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cases = (
        ("negation", "negation", "~LeftCall"),
        ("disjunction", "disjunction", "LeftCall \\/ RightCall"),
        ("implication", "implication", "LeftCall => RightCall"),
        ("negated-implication", "implication", "LeftCall => ~RightCall"),
        ("stacked-negated-implication", "implication", "LeftCall => ~~RightCall"),
        ("unary-temporal-implication", "implication", "LeftCall => [] RightCall"),
        ("literal-gated-negation", "negation", "TRUE /\\ ~LeftCall"),
        ("equivalence", "equivalence", "LeftCall <=> RightCall"),
    )
    for case_name, operator_name, wrapper_body in cases:
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "SumeragiTransitiveParameterizedCallBooleanEnvelope.tla"
        cfg = case_dir / "SumeragiTransitiveParameterizedCallBooleanEnvelope_fast.cfg"
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE SumeragiTransitiveParameterizedCallBooleanEnvelope ----",
                    "CONSTANT Cases",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "LeftPredicate(c) == checked = c",
                    "RightPredicate(c) == ready = c",
                    "LeftCall == LeftPredicate(Cases)",
                    "RightCall == RightPredicate(Cases)",
                    f"ParameterizedCallBooleanWrapper == {wrapper_body}",
                    "TransitiveParameterizedCallBooleanExactness ==",
                    "  /\\ ParameterizedCallBooleanWrapper",
                    "TransitiveParameterizedCallBooleanCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ TransitiveParameterizedCallBooleanExactness",
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
                    "INVARIANT TypeInvariant",
                    "INVARIANT TransitiveParameterizedCallBooleanCorrectnessEnvelope",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "transitive-parameterized-call-boolean-envelope-fast",
            tla,
            cfg,
            "Apalache",
        ) == [
            "transitive-parameterized-call-boolean-envelope-fast: Apalache "
            f"cfg {cfg}:4 references correctness envelope "
            "TransitiveParameterizedCallBooleanCorrectnessEnvelope, but "
            "exactness conjunct TransitiveParameterizedCallBooleanExactness "
            f"at {tla}:12 contains transitive exactness predicate chain with "
            "whole-body parameterized-call boolean-composition helper "
            "ParameterizedCallBooleanWrapper reaches "
            "ParameterizedCallBooleanWrapper through "
            f"ParameterizedCallBooleanWrapper at {tla}:10 is whole-body "
            f"{operator_name} {wrapper_body}; name concrete model predicates "
            "before composing exactness predicate chains"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_call_boolean_composition_with_expression_arguments(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedCallBooleanArgumentEnvelope.tla"
    cfg = tmp_path / "SumeragiParameterizedCallBooleanArgumentEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedCallBooleanArgumentEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LeftPredicate(c) == c = c",
                "RightPredicate(c) == c = c",
                "LeftCall == LeftPredicate(ready = checked)",
                "RightCall == RightPredicate(ready = checked)",
                "ParameterizedCallBooleanWrapper == LeftCall \\/ RightCall",
                "TransitiveParameterizedCallBooleanExactness ==",
                "  /\\ ParameterizedCallBooleanWrapper",
                "TransitiveParameterizedCallBooleanCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveParameterizedCallBooleanExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveParameterizedCallBooleanCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "parameterized-call-boolean-argument-envelope-fast",
        tla,
        cfg,
        "Apalache",
    ) == [
        "parameterized-call-boolean-argument-envelope-fast: Apalache "
        f"cfg {cfg}:4 references correctness envelope "
        "TransitiveParameterizedCallBooleanCorrectnessEnvelope, but "
        "exactness conjunct TransitiveParameterizedCallBooleanExactness "
        f"at {tla}:11 contains transitive exactness predicate chain with "
        "whole-body parameterized-call boolean-composition helper "
        "ParameterizedCallBooleanWrapper reaches "
        "ParameterizedCallBooleanWrapper through "
        f"ParameterizedCallBooleanWrapper at {tla}:9 is whole-body "
        "disjunction LeftCall \\/ RightCall; name concrete model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_zero_arity_parameterized_call_leaf(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiZeroArityParameterizedCallLeafEnvelope.tla"
    cfg = tmp_path / "SumeragiZeroArityParameterizedCallLeafEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiZeroArityParameterizedCallLeafEnvelope ----",
                "CONSTANT Cases, Other",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "ConcretePredicateLeaf == Predicate(Cases)",
                "ParameterizedCallLeafExactness ==",
                "  /\\ ConcretePredicateLeaf",
                "ParameterizedCallLeafCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParameterizedCallLeafExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParameterizedCallLeafCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "parameterized-call-leaf-envelope-fast", tla, cfg, "TLC"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_quantified_boolean_composition_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cases = (
        ("negation", "negation", "~LeftQuantified"),
        ("disjunction", "disjunction", "LeftQuantified \\/ RightQuantified"),
        ("implication", "implication", "LeftQuantified => RightQuantified"),
        (
            "negated-implication",
            "implication",
            "LeftQuantified => ~RightQuantified",
        ),
        (
            "stacked-negated-implication",
            "implication",
            "LeftQuantified => ~~RightQuantified",
        ),
        (
            "unary-temporal-implication",
            "implication",
            "LeftQuantified => [] RightQuantified",
        ),
        ("literal-gated-negation", "negation", "TRUE /\\ ~LeftQuantified"),
        ("equivalence", "equivalence", "LeftQuantified <=> RightQuantified"),
    )
    for case_name, operator_name, wrapper_body in cases:
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "SumeragiTransitiveQuantifiedBooleanEnvelope.tla"
        cfg = case_dir / "SumeragiTransitiveQuantifiedBooleanEnvelope_fast.cfg"
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE SumeragiTransitiveQuantifiedBooleanEnvelope ----",
                    "CONSTANT Cases",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "LeftPredicate(c) == checked = c",
                    "RightPredicate(c) == ready = c",
                    "LeftQuantified == \\A c \\in Cases: LeftPredicate(c)",
                    "RightQuantified == \\A c \\in Cases: RightPredicate(c)",
                    f"QuantifiedBooleanWrapper == {wrapper_body}",
                    "TransitiveQuantifiedBooleanExactness ==",
                    "  /\\ QuantifiedBooleanWrapper",
                    "TransitiveQuantifiedBooleanCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ TransitiveQuantifiedBooleanExactness",
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
                    "INVARIANT TypeInvariant",
                    "INVARIANT TransitiveQuantifiedBooleanCorrectnessEnvelope",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "transitive-quantified-boolean-envelope-fast",
            tla,
            cfg,
            "TLC",
        ) == [
            f"transitive-quantified-boolean-envelope-fast: TLC cfg {cfg}:4 "
            "references correctness envelope "
            "TransitiveQuantifiedBooleanCorrectnessEnvelope, but exactness "
            f"conjunct TransitiveQuantifiedBooleanExactness at {tla}:12 "
            "contains transitive exactness predicate chain with whole-body "
            "quantified-predicate boolean-composition helper "
            "QuantifiedBooleanWrapper reaches QuantifiedBooleanWrapper "
            f"through QuantifiedBooleanWrapper at {tla}:10 is whole-body "
            f"{operator_name} {wrapper_body}; name concrete model predicates "
            "before composing exactness predicate chains"
        ]


def test_cfg_correctness_envelope_shape_errors_allows_quantified_leaf_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiQuantifiedLeafEnvelope.tla"
    cfg = tmp_path / "SumeragiQuantifiedLeafEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiQuantifiedLeafEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "ConcreteQuantifiedLeaf == \\A c \\in Cases: Predicate(c)",
                "QuantifiedLeafExactness ==",
                "  /\\ ConcreteQuantifiedLeaf",
                "QuantifiedLeafCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedLeafExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT QuantifiedLeafCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "quantified-leaf-envelope-fast", tla, cfg, "Apalache"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_existential_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiExistentialQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiExistentialQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiExistentialQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "ExistentialLeaf == \\E c \\in Cases: Predicate(c)",
                "ExistentialQuantifiedExactness ==",
                "  /\\ ExistentialLeaf",
                "ExistentialQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ExistentialQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ExistentialQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "existential-quantified-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        f"existential-quantified-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope ExistentialQuantifiedCorrectnessEnvelope, but "
        f"exactness conjunct ExistentialQuantifiedExactness at {tla}:9 contains "
        "transitive exactness predicate chain with existential quantified helper "
        "ExistentialLeaf reaches \\E c \\in Cases: Predicate(c) through "
        f"ExistentialLeaf at {tla}:7; use universal quantified model "
        "predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_existential_quantified_exactness_operand(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedExistentialQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedExistentialQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedExistentialQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "Predicate(c) == checked = c",
                "NestedHelper ==",
                "  /\\ ConcretePredicate",
                "  /\\ \\E c \\in Cases: Predicate(c)",
                "NestedExistentialQuantifiedExactness ==",
                "  /\\ NestedHelper",
                "NestedExistentialQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NestedExistentialQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NestedExistentialQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-existential-quantified-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        f"nested-existential-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "NestedExistentialQuantifiedCorrectnessEnvelope, but exactness "
        f"conjunct NestedExistentialQuantifiedExactness at {tla}:12 contains "
        "transitive exactness predicate chain with existential quantified "
        "helper NestedHelper reaches \\E c \\in Cases: Predicate(c) through "
        f"NestedHelper at {tla}:9; use universal quantified model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_vacuous_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiVacuousQuantifiedEnvelope_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "INVARIANT VacuousQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for case_name, formula, expected_line in (
        ("literal", "\\A c \\in Cases: TRUE", 6),
        ("unbounded_literal", "\\A c: TRUE", 6),
        ("unbounded_unary_temporal_literal", "\\E c: [] FALSE", 6),
        (
            "chained_let_literal",
            "\\A c \\in Cases: LET first == TRUE second == first IN second",
            6,
        ),
        (
            "chained_let_composed_literal",
            "\\A c \\in Cases: LET first == TRUE second == first /\\ TRUE IN second",
            6,
        ),
        ("self", "\\A c \\in Cases: checked = checked", 6),
        ("self_inequality_hash", "\\A c \\in Cases: checked # checked", 6),
        ("self_inequality_slash", "\\A c \\in Cases: checked /= checked", 6),
        ("self_membership_set", "\\A c \\in Cases: c \\in {c}", 6),
        ("self_membership_domain", "\\A c \\in Cases: c \\in Cases", 6),
        (
            "self_membership_comma_shared_domain",
            "\\A c, d \\in Cases: c \\in Cases",
            6,
        ),
        (
            "self_membership_unary_temporal",
            "\\A c \\in Cases: [] (c \\in {c})",
            6,
        ),
        (
            "self_nonmembership_negated",
            "\\A c \\in Cases: ~(c \\notin {c})",
            6,
        ),
        ("empty_set_nonmembership", "\\A c \\in Cases: c \\notin {}", 6),
        (
            "empty_set_nonmembership_unary_temporal",
            "\\A c \\in Cases: [] (c \\notin {})",
            6,
        ),
        (
            "empty_set_membership_negated",
            "\\A c \\in Cases: ~(c \\in {})",
            6,
        ),
        (
            "self_membership_boolean_composed",
            "\\A c \\in Cases: c \\in Cases /\\ c \\notin {}",
            6,
        ),
        (
            "self_membership_identity_gated",
            "\\A c \\in Cases: TRUE /\\ c \\in Cases",
            6,
        ),
        (
            "self_membership_truth_implication",
            "\\A c \\in Cases: c \\notin Cases => FALSE",
            6,
        ),
        ("empty_domain", "\\A c \\in {}: c = ready", 6),
        (
            "empty_domain_unary_temporal",
            "\\A c \\in {}: [] (c = ready)",
            6,
        ),
        ("singleton_domain_equality", "\\A c \\in {1}: c = 1", 6),
        (
            "singleton_tuple_domain_equality",
            "\\A t \\in {<<1, 2>>}: t = <<1, 2>>",
            6,
        ),
        (
            "singleton_domain_comma_shared_domain",
            "\\A c, d \\in {1}: c = 1",
            6,
        ),
        ("singleton_domain_reversed_equality", "\\A c \\in {1}: 1 = c", 6),
        (
            "singleton_domain_unary_temporal",
            "\\A c \\in {1}: [] (c = 1)",
            6,
        ),
        (
            "singleton_domain_negated_inequality",
            "\\A c \\in {1}: ~(c # 1)",
            6,
        ),
        (
            "singleton_domain_boolean_composed",
            "\\A c \\in {1}: c = 1 /\\ 1 = c",
            6,
        ),
        (
            "singleton_domain_identity_gated",
            "\\A c \\in {1}: TRUE => c = 1",
            6,
        ),
        (
            "singleton_domain_truth_equivalence",
            "\\A c \\in {1}: c # 1 <=> FALSE",
            6,
        ),
    ):
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "SumeragiVacuousQuantifiedEnvelope.tla"
        local_cfg = case_dir / "SumeragiVacuousQuantifiedEnvelope_fast.cfg"
        local_cfg.write_text(cfg.read_text(encoding="utf-8"), encoding="utf-8")
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE SumeragiVacuousQuantifiedEnvelope ----",
                    "CONSTANT Cases",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    f"VacuousLeaf == {formula}",
                    "VacuousQuantifiedExactness ==",
                    "  /\\ VacuousLeaf",
                    "VacuousQuantifiedCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ VacuousQuantifiedExactness",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "vacuous-quantified-envelope-fast",
            tla,
            local_cfg,
            "TLC",
        ) == [
            f"vacuous-quantified-envelope-fast: TLC cfg {local_cfg}:4 "
            "references correctness envelope "
            "VacuousQuantifiedCorrectnessEnvelope, but exactness conjunct "
            f"VacuousQuantifiedExactness at {tla}:8 contains transitive "
            "exactness predicate chain with vacuous quantified helper "
            f"VacuousLeaf reaches {formula} through VacuousLeaf at "
            f"{tla}:{expected_line}; keep literal and self-equality, "
            "self-inequality, empty-domain, singleton-domain, self-membership, or empty-set membership quantified helper bodies "
            "out of exactness predicate chains"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_duplicate_bound_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDuplicateBoundQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiDuplicateBoundQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDuplicateBoundQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate(c) == ready = c",
                "DuplicateBoundLeaf == \\A c, c \\in Cases: ConcretePredicate(c)",
                "DuplicateBoundQuantifiedExactness ==",
                "  /\\ DuplicateBoundLeaf",
                "DuplicateBoundQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DuplicateBoundQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DuplicateBoundQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "duplicate-bound-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"duplicate-bound-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "DuplicateBoundQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"DuplicateBoundQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with duplicate quantified helper binding "
        "DuplicateBoundLeaf reaches \\A c, c \\in Cases: ConcretePredicate(c) "
        f"duplicates bound c through DuplicateBoundLeaf at {tla}:7; bind each "
        "quantified identifier once before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unused_bound_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnusedBoundQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiUnusedBoundQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnusedBoundQuantifiedEnvelope ----",
                "CONSTANT Cases, Other",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate(c) == ready = c",
                "UnusedBoundLeaf == \\A c \\in Cases, d \\in Other: ConcretePredicate(c)",
                "UnusedBoundQuantifiedExactness ==",
                "  /\\ UnusedBoundLeaf",
                "UnusedBoundQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ UnusedBoundQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnusedBoundQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unused-bound-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"unused-bound-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "UnusedBoundQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"UnusedBoundQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with unused quantified helper binding "
        "UnusedBoundLeaf reaches \\A c \\in Cases, d \\in Other: "
        f"ConcretePredicate(c) omits bound d through UnusedBoundLeaf at {tla}:7; use every bound "
        "identifier inside quantified model predicates before composing "
        "exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_control_flow_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiControlFlowQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiControlFlowQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiControlFlowQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == ready = c",
                "ControlFlowLeaf == \\A c \\in Cases: IF c = c THEN Predicate(c) ELSE Predicate(c)",
                "ControlFlowQuantifiedExactness ==",
                "  /\\ ControlFlowLeaf",
                "ControlFlowQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ControlFlowQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ControlFlowQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "control-flow-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"control-flow-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "ControlFlowQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"ControlFlowQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with control-flow quantified helper "
        "ControlFlowLeaf reaches \\A c \\in Cases: IF c = c THEN Predicate(c) "
        "ELSE Predicate(c) uses IF through ControlFlowLeaf at "
        f"{tla}:7; name concrete quantified model predicates instead of "
        "selecting predicates inside quantified helper bodies"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_control_flow_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasQuantifiedControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasQuantifiedControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasQuantifiedControlFlowEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Left(c) == c = c",
                "Right(c) == c # c",
                "LetAliasControlFlowLeaf ==",
                "  \\A c \\in Cases:",
                "    LET selected == IF c = c THEN Left(c) ELSE Right(c) IN selected",
                "LetAliasQuantifiedControlFlowExactness ==",
                "  /\\ LetAliasControlFlowLeaf",
                "LetAliasQuantifiedControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasQuantifiedControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasQuantifiedControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-quantified-control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-quantified-control-flow-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "LetAliasQuantifiedControlFlowCorrectnessEnvelope, but exactness "
        f"conjunct LetAliasQuantifiedControlFlowExactness at {tla}:12 "
        "contains transitive exactness predicate chain with control-flow "
        "quantified helper LetAliasControlFlowLeaf reaches \\A c \\in Cases: "
        "LET selected == IF c = c THEN Left(c) ELSE Right(c) IN selected "
        f"uses IF through LetAliasControlFlowLeaf at {tla}:9; name concrete "
        "quantified model predicates instead of selecting predicates inside "
        "quantified helper bodies"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nontransparent_let_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNonTransparentLetQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiNonTransparentLetQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNonTransparentLetQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "NonTransparentLetLeaf ==",
                "  \\A c \\in Cases:",
                "    LET local(x) == TRUE IN local(c)",
                "NonTransparentLetExactness ==",
                "  /\\ NonTransparentLetLeaf",
                "NonTransparentLetCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NonTransparentLetExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NonTransparentLetCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nontransparent-let-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"nontransparent-let-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "NonTransparentLetCorrectnessEnvelope, but exactness conjunct "
        f"NonTransparentLetExactness at {tla}:10 contains transitive "
        "exactness predicate chain with control-flow quantified helper "
        "NonTransparentLetLeaf reaches \\A c \\in Cases: LET local(x) == "
        f"TRUE IN local(c) uses LET through NonTransparentLetLeaf at {tla}:7; "
        "name concrete quantified model predicates instead of selecting "
        "predicates inside quantified helper bodies"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_negated_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNegatedQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiNegatedQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNegatedQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "NegatedQuantifiedLeaf == ~(\\A c \\in Cases: Predicate(c))",
                "NegatedQuantifiedExactness ==",
                "  /\\ NegatedQuantifiedLeaf",
                "NegatedQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NegatedQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NegatedQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "negated-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"negated-quantified-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope NegatedQuantifiedCorrectnessEnvelope, but "
        f"exactness conjunct NegatedQuantifiedExactness at {tla}:9 contains "
        "transitive exactness predicate chain with negated quantified helper "
        "NegatedQuantifiedLeaf reaches \\A c \\in Cases: Predicate(c) under "
        f"1 top-level negation(s) through NegatedQuantifiedLeaf at {tla}:7; "
        "compose positive quantified model predicates before exactness "
        "predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_negated_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetAliasNegatedQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiLetAliasNegatedQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetAliasNegatedQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "LetAliasNegatedQuantifiedLeaf == ~(LET selected == \\A c \\in Cases: Predicate(c) IN selected)",
                "LetAliasNegatedQuantifiedExactness ==",
                "  /\\ LetAliasNegatedQuantifiedLeaf",
                "LetAliasNegatedQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LetAliasNegatedQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LetAliasNegatedQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-alias-negated-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"let-alias-negated-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "LetAliasNegatedQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"LetAliasNegatedQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with negated quantified helper "
        "LetAliasNegatedQuantifiedLeaf reaches \\A c \\in Cases: Predicate(c) "
        f"under 1 top-level negation(s) through LetAliasNegatedQuantifiedLeaf "
        f"at {tla}:7; compose positive quantified model predicates before "
        "exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_negated_quantified_exactness_operand(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedNegatedQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedNegatedQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedNegatedQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "Predicate(c) == checked = c",
                "NestedHelper ==",
                "  /\\ ConcretePredicate",
                "  /\\ ~(\\A c \\in Cases: Predicate(c))",
                "NestedNegatedQuantifiedExactness ==",
                "  /\\ NestedHelper",
                "NestedNegatedQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NestedNegatedQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NestedNegatedQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-negated-quantified-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"nested-negated-quantified-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "NestedNegatedQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"NestedNegatedQuantifiedExactness at {tla}:12 contains transitive "
        "exactness predicate chain with negated quantified helper "
        "NestedHelper reaches \\A c \\in Cases: Predicate(c) under "
        f"1 top-level negation(s) through NestedHelper at {tla}:9; "
        "compose positive quantified model predicates before exactness "
        "predicate chains"
    ]


def test_undefined_static_helper_identifiers_ignore_quantifier_context(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiQuantifiedIdentifierScan.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiQuantifiedIdentifierScan ----",
                "CONSTANT Cases",
                "Known(Case) == checked = Case",
                "Bits(Case) == <<>>",
                "ConcreteRecord == [Present |-> TRUE]",
                "OtherRecord == [Present |-> TRUE]",
                "ParamHelper(MissingHelper) == Known(MissingHelper) /\\ MissingHelper",
                "ParamFree(Alias) == Alias /\\ FreeMissing",
                "====",
            ]
        ),
        encoding="utf-8",
    )
    definitions = module.tla_single_expression_operator_definitions(tla)
    parameter_names = module.tla_operator_parameter_names(tla)
    formula = (
        '\\A Case \\in Cases: /\\ Known(Case) /\\ Len(Bits(Case)) >= 0 '
        '/\\ label = "MissingInString" /\\ MissingHelper'
    )

    assert parameter_names["Known"] == frozenset({"Case"})
    assert parameter_names["ParamHelper"] == frozenset({"MissingHelper"})
    assert parameter_names["ParamFree"] == frozenset({"Alias"})
    assert module.tla_quantified_bound_identifiers(formula) == {"Case"}
    assert "MissingInString" not in module.tla_static_identifiers(
        module.tla_without_string_literals(formula)
    )
    assert module.undefined_static_helper_identifiers(
        formula,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    unbounded_quantifier_bound_only = (
        "\\A MissingHelper, OtherHelper: "
        "MissingHelper /\\ OtherHelper /\\ Known"
    )
    assert module.tla_quantified_bound_identifiers(
        unbounded_quantifier_bound_only
    ) == {
        "MissingHelper",
        "OtherHelper",
    }
    assert module.tla_quantifier_scope(unbounded_quantifier_bound_only) == (
        [],
        "MissingHelper /\\ OtherHelper /\\ Known",
        {"MissingHelper", "OtherHelper"},
    )
    assert module.tla_free_static_identifiers(
        unbounded_quantifier_bound_only
    ) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        unbounded_quantifier_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    unbounded_quantifier_body_free = (
        "\\E <<MissingLeft, MissingRight>>: "
        "MissingLeft /\\ MissingRight /\\ FreeMissing"
    )
    assert module.tla_quantified_bound_identifiers(
        unbounded_quantifier_body_free
    ) == {
        "MissingLeft",
        "MissingRight",
    }
    assert module.tla_free_static_identifiers(unbounded_quantifier_body_free) == {
        "FreeMissing"
    }
    assert module.undefined_static_helper_identifiers(
        unbounded_quantifier_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    quantifier_subset_relation_prefix_not_bound = (
        "\\A MissingHelper \\subseteq MissingDomain: "
        "MissingHelper /\\ FreeMissing"
    )
    assert module.tla_quantifier_scope(
        quantifier_subset_relation_prefix_not_bound
    ) is None
    assert module.tla_quantified_bound_identifiers(
        quantifier_subset_relation_prefix_not_bound
    ) == set()
    assert module.tla_free_static_identifiers(
        quantifier_subset_relation_prefix_not_bound
    ) == {
        "FreeMissing",
        "MissingDomain",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        quantifier_subset_relation_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingDomain", "MissingHelper"]

    quantifier_nonmembership_prefix_not_bound = (
        "\\E MissingHelper \\notin MissingDomain: "
        "MissingHelper /\\ FreeMissing"
    )
    assert module.tla_quantifier_scope(
        quantifier_nonmembership_prefix_not_bound
    ) is None
    assert module.tla_free_static_identifiers(
        quantifier_nonmembership_prefix_not_bound
    ) == {
        "FreeMissing",
        "MissingDomain",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        quantifier_nonmembership_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingDomain", "MissingHelper"]

    quantifier_infix_prefix_not_bound = (
        "\\A MissingHelper \\cup MissingDomain: "
        "MissingHelper /\\ FreeMissing"
    )
    assert module.tla_binding_identifiers_from_names(
        "MissingHelper \\cup MissingDomain"
    ) == set()
    assert module.tla_binding_identifiers_from_names(
        "<<MissingLeft, MissingRight>>"
    ) == {
        "MissingLeft",
        "MissingRight",
    }
    assert module.tla_quantifier_scope(quantifier_infix_prefix_not_bound) is None
    assert module.tla_quantified_bound_identifiers(
        quantifier_infix_prefix_not_bound
    ) == set()
    assert module.tla_free_static_identifiers(
        quantifier_infix_prefix_not_bound
    ) == {
        "FreeMissing",
        "MissingDomain",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        quantifier_infix_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingDomain", "MissingHelper"]

    standard_tla_string_set = "Known(Case) /\\ Case \\in STRING"
    assert module.tla_free_static_identifiers(standard_tla_string_set) == {
        "Case",
        "Known",
        "STRING",
    }
    assert module.undefined_static_helper_identifiers(
        standard_tla_string_set,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
        local_bound=frozenset({"Case"}),
    ) == []

    standard_tla_operator_mix = (
        "MissingHelper \\in STRING /\\ Known \\in BOOLEAN /\\ "
        "Cardinality(SUBSET Cases) >= Len(Seq(Known))"
    )
    assert module.undefined_static_helper_identifiers(
        standard_tla_operator_mix,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    enabled_selector_field_bound_only = "ENABLED (Known.MissingField)"
    assert module.tla_unary_action_operand(enabled_selector_field_bound_only) == (
        "Known.MissingField"
    )
    assert module.tla_free_static_identifiers(enabled_selector_field_bound_only) == {
        "Known"
    }
    assert module.undefined_static_helper_identifiers(
        enabled_selector_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    enabled_selector_body_free = (
        "ENABLED (Known.MissingField /\\ FreeMissing)"
    )
    assert module.tla_free_static_identifiers(enabled_selector_body_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        enabled_selector_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    unchanged_tuple_field_bound_only = "UNCHANGED <<Known.MissingField>>"
    assert module.tla_unary_action_operand(unchanged_tuple_field_bound_only) == (
        "<<Known.MissingField>>"
    )
    assert module.tla_tuple_literal_values("<<Known.MissingField>>") == [
        "Known.MissingField"
    ]
    assert module.tla_free_static_identifiers(
        unchanged_tuple_field_bound_only
    ) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        unchanged_tuple_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    unchanged_tuple_body_free = (
        "UNCHANGED <<Known.MissingField, FreeMissing>>"
    )
    assert module.tla_tuple_literal_values(
        "<<Known.MissingField, FreeMissing>>"
    ) == ["Known.MissingField", "FreeMissing"]
    assert module.tla_free_static_identifiers(unchanged_tuple_body_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        unchanged_tuple_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    case_selector_field_bound_only = (
        "CASE Known -> Known.MissingField [] OTHER -> Known"
    )
    assert module.tla_top_level_case_condition_result_branches(
        case_selector_field_bound_only
    ) == [
        ("Known", "Known.MissingField"),
        ("OTHER", "Known"),
    ]
    assert module.tla_free_static_identifiers(case_selector_field_bound_only) == {
        "Known"
    }
    assert module.undefined_static_helper_identifiers(
        case_selector_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    case_branch_body_free = (
        "CASE Known -> Known.MissingField /\\ FreeMissing [] OTHER -> Known"
    )
    assert module.tla_top_level_case_result_branches(case_branch_body_free) == [
        "Known.MissingField /\\ FreeMissing",
        "Known",
    ]
    assert module.tla_free_static_identifiers(case_branch_body_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        case_branch_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    case_record_field_bound_only = (
        "CASE Known -> [MissingField |-> Known] "
        "[] OTHER -> [OtherField |-> FreeMissing]"
    )
    assert module.tla_free_static_identifiers(case_record_field_bound_only) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        case_record_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    relation_selector_field_bound_only = "ConcreteRecord.MissingField = TRUE"
    assert module.tla_top_level_relation_parts(
        relation_selector_field_bound_only
    ) == ("ConcreteRecord.MissingField", "=", "TRUE")
    assert module.tla_free_static_identifiers(
        relation_selector_field_bound_only
    ) == {"ConcreteRecord"}
    assert module.undefined_static_helper_identifiers(
        relation_selector_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    relation_selector_body_free = "ConcreteRecord.MissingField # FreeMissing"
    assert module.tla_top_level_relation_parts(relation_selector_body_free) == (
        "ConcreteRecord.MissingField",
        "#",
        "FreeMissing",
    )
    assert module.tla_free_static_identifiers(relation_selector_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        relation_selector_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    relation_order_field_bound_only = "ConcreteRecord.MissingField < 1"
    assert module.tla_top_level_relation_parts(
        relation_order_field_bound_only
    ) == ("ConcreteRecord.MissingField", "<", "1")
    assert module.tla_free_static_identifiers(
        relation_order_field_bound_only
    ) == {"ConcreteRecord"}
    assert module.undefined_static_helper_identifiers(
        relation_order_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    call_argument_field_bound_only = "Known(ConcreteRecord.MissingField)"
    assert module.tla_free_static_identifiers(call_argument_field_bound_only) == {
        "ConcreteRecord",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        call_argument_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    call_argument_body_free = (
        "Known([MissingField |-> ConcreteRecord.MissingField]) /\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(call_argument_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        call_argument_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    missing_call_argument_field_bound_only = (
        "MissingHelper(ConcreteRecord.MissingField)"
    )
    assert module.tla_free_static_identifiers(
        missing_call_argument_field_bound_only
    ) == {"ConcreteRecord", "MissingHelper"}
    assert module.undefined_static_helper_identifiers(
        missing_call_argument_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    arithmetic_infix_field_bound_only = "ConcreteRecord.MissingField + 1"
    assert module.tla_top_level_static_infix_operands(
        arithmetic_infix_field_bound_only
    ) == ["ConcreteRecord.MissingField", "1"]
    assert module.tla_free_static_identifiers(arithmetic_infix_field_bound_only) == {
        "ConcreteRecord"
    }
    assert module.undefined_static_helper_identifiers(
        arithmetic_infix_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    arithmetic_infix_body_free = (
        "ConcreteRecord.MissingField + OtherRecord.OtherField + FreeMissing"
    )
    assert module.tla_top_level_static_infix_operands(
        arithmetic_infix_body_free
    ) == [
        "ConcreteRecord.MissingField",
        "OtherRecord.OtherField",
        "FreeMissing",
    ]
    assert module.tla_free_static_identifiers(arithmetic_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
        "OtherRecord",
    }
    assert module.undefined_static_helper_identifiers(
        arithmetic_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    set_infix_body_free = "ConcreteRecord.MissingField \\cup {FreeMissing}"
    assert module.tla_top_level_static_infix_operands(set_infix_body_free) == [
        "ConcreteRecord.MissingField",
        "{FreeMissing}",
    ]
    assert module.tla_free_static_identifiers(set_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        set_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    set_synonym_infix_body_free = (
        "ConcreteRecord.MissingField \\union OtherRecord.OtherField "
        "\\setminus {FreeMissing}"
    )
    assert module.tla_top_level_static_infix_operands(
        set_synonym_infix_body_free
    ) == [
        "ConcreteRecord.MissingField",
        "OtherRecord.OtherField",
        "{FreeMissing}",
    ]
    assert module.tla_free_static_identifiers(set_synonym_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
        "OtherRecord",
    }
    assert module.undefined_static_helper_identifiers(
        set_synonym_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    math_infix_body_free = (
        "ConcreteRecord.MissingField ^ OtherRecord.OtherField \\cdot FreeMissing"
    )
    assert module.tla_top_level_static_infix_operands(math_infix_body_free) == [
        "ConcreteRecord.MissingField",
        "OtherRecord.OtherField",
        "FreeMissing",
    ]
    assert module.tla_free_static_identifiers(math_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
        "OtherRecord",
    }
    assert module.undefined_static_helper_identifiers(
        math_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    sequence_infix_body_free = "ConcreteRecord.MissingField \\o <<FreeMissing>>"
    assert module.tla_top_level_static_infix_operands(sequence_infix_body_free) == [
        "ConcreteRecord.MissingField",
        "<<FreeMissing>>",
    ]
    assert module.tla_free_static_identifiers(sequence_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        sequence_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    override_infix_field_bound_only = (
        "ConcreteRecord.MissingField @@ OtherRecord.OtherField"
    )
    assert module.tla_top_level_static_infix_operands(
        override_infix_field_bound_only
    ) == [
        "ConcreteRecord.MissingField",
        "OtherRecord.OtherField",
    ]
    assert module.tla_free_static_identifiers(override_infix_field_bound_only) == {
        "ConcreteRecord",
        "OtherRecord",
    }
    assert module.undefined_static_helper_identifiers(
        override_infix_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    maplet_infix_body_free = "ConcreteRecord.MissingField :> FreeMissing"
    assert module.tla_top_level_relation_parts(maplet_infix_body_free) is None
    assert module.tla_top_level_static_infix_operands(maplet_infix_body_free) == [
        "ConcreteRecord.MissingField",
        "FreeMissing",
    ]
    assert module.tla_free_static_identifiers(maplet_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        maplet_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    subset_relation_field_bound_only = (
        "ConcreteRecord.MissingField \\subseteq OtherRecord.OtherField"
    )
    assert module.tla_top_level_relation_parts(subset_relation_field_bound_only) == (
        "ConcreteRecord.MissingField",
        "\\subseteq",
        "OtherRecord.OtherField",
    )
    assert module.tla_free_static_identifiers(subset_relation_field_bound_only) == {
        "ConcreteRecord",
        "OtherRecord",
    }
    assert module.undefined_static_helper_identifiers(
        subset_relation_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    call_argument_infix_body_free = (
        "Known(ConcreteRecord.MissingField + FreeMissing)"
    )
    assert module.tla_free_static_identifiers(call_argument_infix_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        call_argument_infix_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    _, param_bound_body = definitions["ParamHelper"]
    assert module.undefined_static_helper_identifiers(
        param_bound_body,
        definitions,
        tla,
        current="ParamHelper",
        exactness_operator="IdentifierScanExactness",
        local_bound=parameter_names["ParamHelper"],
    ) == []

    _, param_free_body = definitions["ParamFree"]
    assert module.undefined_static_helper_identifiers(
        param_free_body,
        definitions,
        tla,
        current="ParamFree",
        exactness_operator="IdentifierScanExactness",
        local_bound=parameter_names["ParamFree"],
    ) == ["FreeMissing"]

    nested_bound_only = (
        "\\A Case \\in Cases: \\A MissingHelper \\in Cases: Known(MissingHelper)"
    )
    assert module.undefined_static_helper_identifiers(
        nested_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    nested_shadowed_free = (
        "\\A Case \\in Cases: "
        "(\\A MissingHelper \\in Cases: Known(MissingHelper)) /\\ MissingHelper"
    )
    assert module.undefined_static_helper_identifiers(
        nested_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    tuple_pattern_domain_free = (
        "\\A <<MissingLeft, MissingRight>> \\in MissingDomain: "
        "Known /\\ MissingLeft /\\ MissingRight"
    )
    assert module.tla_free_static_identifiers(tuple_pattern_domain_free) == {
        "Known",
        "MissingDomain",
    }
    assert module.undefined_static_helper_identifiers(
        tuple_pattern_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingDomain"]

    let_bound_only = "LET MissingHelper == Known IN MissingHelper"
    assert module.tla_free_static_identifiers(let_bound_only) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        let_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    let_shadowed_free = "LET MissingHelper == Known IN MissingHelper /\\ FreeMissing"
    assert module.tla_free_static_identifiers(let_shadowed_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        let_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    let_operand_free = "LET Alias == MissingHelper IN Alias"
    assert module.tla_free_static_identifiers(let_operand_free) == {
        "MissingHelper"
    }
    assert module.undefined_static_helper_identifiers(
        let_operand_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    let_parameterized_bound_only = (
        "LET LocalHelper(MissingParam) == Known /\\ MissingParam "
        "IN LocalHelper(1)"
    )
    let_parameterized_entries = module.tla_static_let_binding_entries(
        "LocalHelper(MissingParam) == Known /\\ MissingParam"
    )
    assert len(let_parameterized_entries) == 1
    assert let_parameterized_entries[0].name == "LocalHelper"
    assert let_parameterized_entries[0].params == frozenset({"MissingParam"})
    assert module.tla_static_let_binding_definitions(
        "LocalHelper(MissingParam) == Known /\\ MissingParam"
    ) is None
    assert module.tla_free_static_identifiers(let_parameterized_bound_only) == {
        "Known"
    }
    assert module.undefined_static_helper_identifiers(
        let_parameterized_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    let_parameterized_body_free = (
        "LET LocalHelper(MissingParam) == Known /\\ MissingParam /\\ FreeMissing "
        "IN LocalHelper(1)"
    )
    assert module.tla_free_static_identifiers(let_parameterized_body_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        let_parameterized_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    let_parameterized_argument_free = (
        "LET LocalHelper(MissingParam) == Known /\\ MissingParam "
        "IN LocalHelper(ResultMissing)"
    )
    assert module.tla_free_static_identifiers(let_parameterized_argument_free) == {
        "Known",
        "ResultMissing",
    }
    assert module.undefined_static_helper_identifiers(
        let_parameterized_argument_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["ResultMissing"]

    lambda_bound_only = "LAMBDA MissingHelper \\in Cases: Known /\\ MissingHelper"
    assert module.tla_lambda_scope(lambda_bound_only) == (
        ["Cases"],
        "Known /\\ MissingHelper",
        {"MissingHelper"},
    )
    assert module.tla_free_static_identifiers(lambda_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    lambda_comma_bound_only = (
        "LAMBDA MissingLeft, MissingRight \\in Cases: "
        "Known /\\ MissingLeft /\\ MissingRight"
    )
    assert module.tla_lambda_scope(lambda_comma_bound_only) == (
        ["Cases"],
        "Known /\\ MissingLeft /\\ MissingRight",
        {"MissingLeft", "MissingRight"},
    )
    assert module.tla_free_static_identifiers(lambda_comma_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_comma_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    lambda_tuple_bound_only = (
        "LAMBDA <<MissingLeft, MissingRight>> \\in Cases: "
        "Known /\\ MissingLeft /\\ MissingRight"
    )
    assert module.tla_lambda_scope(lambda_tuple_bound_only) == (
        ["Cases"],
        "Known /\\ MissingLeft /\\ MissingRight",
        {"MissingLeft", "MissingRight"},
    )
    assert module.tla_free_static_identifiers(lambda_tuple_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_tuple_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    lambda_domain_free = "LAMBDA Alias \\in MissingDomain: Alias"
    assert module.tla_lambda_scope(lambda_domain_free) == (
        ["MissingDomain"],
        "Alias",
        {"Alias"},
    )
    assert module.tla_free_static_identifiers(lambda_domain_free) == {
        "MissingDomain"
    }
    assert module.undefined_static_helper_identifiers(
        lambda_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingDomain"]

    lambda_body_free = (
        "LAMBDA MissingHelper \\in Cases: MissingHelper /\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(lambda_body_free) == {
        "Cases",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    lambda_plain_bound_free = "LAMBDA MissingHelper: MissingHelper /\\ FreeMissing"
    assert module.tla_lambda_scope(lambda_plain_bound_free) == (
        [],
        "MissingHelper /\\ FreeMissing",
        {"MissingHelper"},
    )
    assert module.tla_free_static_identifiers(lambda_plain_bound_free) == {
        "FreeMissing"
    }
    assert module.undefined_static_helper_identifiers(
        lambda_plain_bound_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    lambda_subset_relation_prefix_not_bound = (
        "LAMBDA MissingHelper \\subseteq Cases: MissingHelper /\\ FreeMissing"
    )
    assert module.tla_lambda_scope(lambda_subset_relation_prefix_not_bound) is None
    assert module.tla_free_static_identifiers(
        lambda_subset_relation_prefix_not_bound
    ) == {
        "Cases",
        "FreeMissing",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_subset_relation_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingHelper"]

    lambda_equality_relation_prefix_not_bound = (
        "LAMBDA MissingHelper # Cases: MissingHelper /\\ FreeMissing"
    )
    assert module.tla_lambda_scope(lambda_equality_relation_prefix_not_bound) is None
    assert module.tla_free_static_identifiers(
        lambda_equality_relation_prefix_not_bound
    ) == {
        "Cases",
        "FreeMissing",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_equality_relation_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingHelper"]

    lambda_application_argument_free = (
        "(LAMBDA MissingHelper \\in Cases: MissingHelper)[Known]"
    )
    assert module.tla_lambda_scope(lambda_application_argument_free) is None
    assert module.tla_free_static_identifiers(lambda_application_argument_free) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        lambda_application_argument_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    choose_bound_only = "CHOOSE MissingHelper \\in Cases: Known /\\ MissingHelper"
    assert module.tla_free_static_identifiers(choose_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        choose_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    choose_shadowed_free = (
        "(CHOOSE MissingHelper \\in Cases: Known /\\ MissingHelper) "
        "/\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(choose_shadowed_free) == {
        "Cases",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        choose_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    choose_domain_free = "CHOOSE Alias \\in MissingHelper: Alias"
    assert module.tla_free_static_identifiers(choose_domain_free) == {
        "MissingHelper"
    }
    assert module.undefined_static_helper_identifiers(
        choose_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    choose_subset_relation_prefix_not_bound = (
        "CHOOSE MissingHelper \\subseteq Cases: MissingHelper /\\ FreeMissing"
    )
    assert module.tla_choose_binding_identifiers(
        "CHOOSE MissingHelper \\subseteq Cases"
    ) is None
    assert module.tla_free_static_identifiers(
        choose_subset_relation_prefix_not_bound
    ) == {
        "Cases",
        "FreeMissing",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        choose_subset_relation_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingHelper"]

    choose_nonmembership_prefix_not_bound = (
        "CHOOSE MissingHelper \\notin Cases: MissingHelper /\\ FreeMissing"
    )
    assert module.tla_choose_binding_identifiers(
        "CHOOSE MissingHelper \\notin Cases"
    ) is None
    assert module.tla_free_static_identifiers(
        choose_nonmembership_prefix_not_bound
    ) == {
        "Cases",
        "FreeMissing",
        "MissingHelper",
    }
    assert module.undefined_static_helper_identifiers(
        choose_nonmembership_prefix_not_bound,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingHelper"]

    set_bound_only = "{MissingHelper \\in Cases: Known /\\ MissingHelper}"
    assert module.tla_free_static_identifiers(set_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        set_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    set_compound_bound_only = (
        "{MissingHelper \\in Cases: Known /\\ MissingHelper} /\\ "
        "{OtherMissing \\in Cases: OtherMissing}"
    )
    assert module.tla_set_comprehension_scope(set_compound_bound_only) is None
    assert module.tla_explicit_set_elements(set_compound_bound_only) is None
    assert module.tla_free_static_identifiers(set_compound_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        set_compound_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    set_compound_body_free = (
        "{MissingHelper \\in Cases: Known /\\ MissingHelper} /\\ "
        "{OtherMissing \\in Cases: OtherMissing} /\\ FreeMissing"
    )
    assert module.tla_set_comprehension_scope(set_compound_body_free) is None
    assert module.tla_free_static_identifiers(set_compound_body_free) == {
        "Cases",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        set_compound_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    set_shadowed_free = (
        "{MissingHelper \\in Cases: Known /\\ MissingHelper} /\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(set_shadowed_free) == {
        "Cases",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        set_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    set_domain_free = "{Alias \\in MissingHelper: Alias}"
    assert module.tla_free_static_identifiers(set_domain_free) == {
        "MissingHelper"
    }
    assert module.undefined_static_helper_identifiers(
        set_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    set_comma_bound_only = (
        "{MissingLeft, MissingRight \\in Cases: "
        "Known /\\ MissingLeft /\\ MissingRight}"
    )
    assert module.tla_free_static_identifiers(set_comma_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        set_comma_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    set_comma_shadowed_free = (
        "{MissingLeft, MissingRight \\in Cases: "
        "Known /\\ MissingLeft /\\ MissingRight} /\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(set_comma_shadowed_free) == {
        "Cases",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        set_comma_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    explicit_set_record_field_bound_only = "{[MissingField |-> Known]}"
    assert module.tla_explicit_set_elements(
        explicit_set_record_field_bound_only
    ) == ["[MissingField |-> Known]"]
    assert module.tla_free_static_identifiers(
        explicit_set_record_field_bound_only
    ) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        explicit_set_record_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    explicit_set_selector_body_free = "{ConcreteRecord.MissingField, FreeMissing}"
    assert module.tla_explicit_set_elements(explicit_set_selector_body_free) == [
        "ConcreteRecord.MissingField",
        "FreeMissing",
    ]
    assert module.tla_free_static_identifiers(explicit_set_selector_body_free) == {
        "ConcreteRecord",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        explicit_set_selector_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    unary_domain_record_field_bound_only = "DOMAIN [MissingField |-> Known]"
    assert module.tla_unary_set_operator_operand(
        unary_domain_record_field_bound_only
    ) == "[MissingField |-> Known]"
    assert module.tla_free_static_identifiers(
        unary_domain_record_field_bound_only
    ) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        unary_domain_record_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    unary_subset_record_set_field_bound_only = "SUBSET [MissingField: Known]"
    assert module.tla_unary_set_operator_operand(
        unary_subset_record_set_field_bound_only
    ) == "[MissingField: Known]"
    assert module.tla_free_static_identifiers(
        unary_subset_record_set_field_bound_only
    ) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        unary_subset_record_set_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    unary_subset_comprehension_bound_only = (
        "SUBSET {MissingHelper \\in Cases: Known /\\ MissingHelper}"
    )
    assert module.tla_free_static_identifiers(
        unary_subset_comprehension_bound_only
    ) == {"Cases", "Known"}
    assert module.undefined_static_helper_identifiers(
        unary_subset_comprehension_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    unary_union_literal_body_free = "UNION {[MissingField |-> FreeMissing]}"
    assert module.tla_unary_set_operator_operand(
        unary_union_literal_body_free
    ) == "{[MissingField |-> FreeMissing]}"
    assert module.tla_free_static_identifiers(unary_union_literal_body_free) == {
        "FreeMissing"
    }
    assert module.undefined_static_helper_identifiers(
        unary_union_literal_body_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    function_bound_only = "[MissingHelper \\in Cases |-> Known /\\ MissingHelper]"
    assert module.tla_free_static_identifiers(function_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        function_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    function_shadowed_free = (
        "[MissingHelper \\in Cases |-> Known /\\ MissingHelper] /\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(function_shadowed_free) == {
        "Cases",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        function_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    function_domain_free = "[Alias \\in MissingHelper |-> Alias]"
    assert module.tla_free_static_identifiers(function_domain_free) == {
        "MissingHelper"
    }
    assert module.undefined_static_helper_identifiers(
        function_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingHelper"]

    function_set_record_range_bound_only = "[Cases -> [MissingField |-> Known]]"
    assert module.tla_function_set_scope(function_set_record_range_bound_only) == (
        "Cases",
        "[MissingField |-> Known]",
    )
    assert module.tla_function_set_scope(
        "[Cases -> CASE Ready -> Known [] OTHER -> Known]"
    ) == (
        "Cases",
        "CASE Ready -> Known [] OTHER -> Known",
    )
    assert module.tla_function_set_scope(
        "[CASE Ready -> Known [] OTHER -> Known -> Cases]"
    ) == (
        "CASE Ready -> Known [] OTHER -> Known",
        "Cases",
    )
    assert module.tla_function_set_scope("[MissingField |-> Known]") is None
    assert (
        module.tla_function_set_scope(
            "[MissingField |-> CASE Ready -> Known [] OTHER -> Known]"
        )
        is None
    )
    assert (
        module.tla_function_set_scope(
            "[MissingField: CASE Ready -> Known [] OTHER -> Known]"
        )
        is None
    )
    assert (
        module.tla_function_set_scope(
            "[record EXCEPT !.field = CASE Ready -> Known [] OTHER -> Known]"
        )
        is None
    )
    assert module.tla_free_static_identifiers(
        function_set_record_range_bound_only
    ) == {"Cases", "Known"}
    assert module.undefined_static_helper_identifiers(
        function_set_record_range_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    function_set_domain_free = "[MissingDomain -> [MissingField |-> Known]]"
    assert module.tla_free_static_identifiers(function_set_domain_free) == {
        "Known",
        "MissingDomain",
    }
    assert module.undefined_static_helper_identifiers(
        function_set_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingDomain"]

    function_set_range_free = "[Cases -> [MissingField |-> FreeMissing]]"
    assert module.tla_free_static_identifiers(function_set_range_free) == {
        "Cases",
        "FreeMissing",
    }
    assert module.undefined_static_helper_identifiers(
        function_set_range_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    function_set_constructor_range_bound_only = (
        "[Cases -> [MissingHelper \\in Cases |-> MissingHelper]]"
    )
    assert module.tla_free_static_identifiers(
        function_set_constructor_range_bound_only
    ) == {"Cases"}
    assert module.undefined_static_helper_identifiers(
        function_set_constructor_range_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    function_set_unary_set_range_free = (
        "[Cases -> DOMAIN [MissingField |-> FreeMissing]]"
    )
    assert module.tla_free_static_identifiers(
        function_set_unary_set_range_free
    ) == {"Cases", "FreeMissing"}
    assert module.undefined_static_helper_identifiers(
        function_set_unary_set_range_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    record_field_bound_only = "[MissingField |-> Known]"
    assert module.tla_record_literal_values(record_field_bound_only) == ["Known"]
    assert module.tla_free_static_identifiers(record_field_bound_only) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        record_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    record_value_free = "[MissingField |-> Known, OtherField |-> FreeMissing]"
    assert module.tla_record_literal_values(record_value_free) == [
        "Known",
        "FreeMissing",
    ]
    assert module.tla_free_static_identifiers(record_value_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        record_value_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    record_set_field_bound_only = "[MissingField: Known]"
    assert module.tla_record_set_field_domains(record_set_field_bound_only) == ["Known"]
    assert module.tla_free_static_identifiers(record_set_field_bound_only) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        record_set_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    record_set_domain_free = "[MissingField: FreeMissing, OtherField: Known]"
    assert module.tla_record_set_field_domains(record_set_domain_free) == [
        "FreeMissing",
        "Known",
    ]
    assert module.tla_free_static_identifiers(record_set_domain_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        record_set_domain_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    record_update_field_bound_only = "[Known EXCEPT !.MissingField = TRUE]"
    assert module.tla_record_update_scope(record_update_field_bound_only) == (
        "Known",
        [],
        ["TRUE"],
    )
    assert module.tla_free_static_identifiers(record_update_field_bound_only) == {
        "Known"
    }
    assert module.undefined_static_helper_identifiers(
        record_update_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    record_update_value_free = "[Known EXCEPT !.MissingField = FreeMissing]"
    assert module.tla_record_update_scope(record_update_value_free) == (
        "Known",
        [],
        ["FreeMissing"],
    )
    assert module.tla_free_static_identifiers(record_update_value_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        record_update_value_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    record_update_index_free = (
        "[Known EXCEPT ![MissingIndex].MissingField = FreeMissing]"
    )
    assert module.tla_record_update_scope(record_update_index_free) == (
        "Known",
        ["MissingIndex"],
        ["FreeMissing"],
    )
    assert module.tla_free_static_identifiers(record_update_index_free) == {
        "FreeMissing",
        "Known",
        "MissingIndex",
    }
    assert module.undefined_static_helper_identifiers(
        record_update_index_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing", "MissingIndex"]

    record_update_at_value = "[Known EXCEPT !.MissingField = @]"
    assert module.tla_record_update_scope(record_update_at_value) == (
        "Known",
        [],
        ["@"],
    )
    assert module.tla_free_static_identifiers(record_update_at_value) == {"Known"}
    assert module.undefined_static_helper_identifiers(
        record_update_at_value,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    record_selector_field_bound_only = "Known.MissingField"
    assert module.tla_selector_scope(record_selector_field_bound_only) == (
        "Known",
        [],
    )
    assert module.tla_free_static_identifiers(record_selector_field_bound_only) == {
        "Known"
    }
    assert module.undefined_static_helper_identifiers(
        record_selector_field_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    record_selector_chained_fields = "Known.MissingField.OtherField"
    assert module.tla_selector_scope(record_selector_chained_fields) == (
        "Known",
        [],
    )
    assert module.tla_free_static_identifiers(record_selector_chained_fields) == {
        "Known"
    }
    assert module.undefined_static_helper_identifiers(
        record_selector_chained_fields,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    record_selector_index_free = "Known[MissingIndex].MissingField"
    assert module.tla_selector_scope(record_selector_index_free) == (
        "Known",
        ["MissingIndex"],
    )
    assert module.tla_free_static_identifiers(record_selector_index_free) == {
        "Known",
        "MissingIndex",
    }
    assert module.undefined_static_helper_identifiers(
        record_selector_index_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingIndex"]

    record_selector_field_then_index_free = "Known.MissingField[MissingIndex]"
    assert module.tla_selector_scope(record_selector_field_then_index_free) == (
        "Known",
        ["MissingIndex"],
    )
    assert module.tla_free_static_identifiers(
        record_selector_field_then_index_free
    ) == {"Known", "MissingIndex"}
    assert module.undefined_static_helper_identifiers(
        record_selector_field_then_index_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingIndex"]

    record_selector_nested_index_field = "Known[MissingRecord.MissingField]"
    assert module.tla_selector_scope(record_selector_nested_index_field) == (
        "Known",
        ["MissingRecord.MissingField"],
    )
    assert module.tla_free_static_identifiers(record_selector_nested_index_field) == {
        "Known",
        "MissingRecord",
    }
    assert module.undefined_static_helper_identifiers(
        record_selector_nested_index_field,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["MissingRecord"]

    record_selector_shadowed_free = "Known.MissingField /\\ FreeMissing"
    assert module.tla_selector_scope(record_selector_shadowed_free) is None
    assert module.tla_free_static_identifiers(record_selector_shadowed_free) == {
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        record_selector_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]

    function_comma_bound_only = (
        "[MissingLeft, MissingRight \\in Cases |-> "
        "Known /\\ MissingLeft /\\ MissingRight]"
    )
    assert module.tla_free_static_identifiers(function_comma_bound_only) == {
        "Cases",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        function_comma_bound_only,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == []

    function_comma_shadowed_free = (
        "[MissingLeft, MissingRight \\in Cases |-> "
        "Known /\\ MissingLeft /\\ MissingRight] /\\ FreeMissing"
    )
    assert module.tla_free_static_identifiers(function_comma_shadowed_free) == {
        "Cases",
        "FreeMissing",
        "Known",
    }
    assert module.undefined_static_helper_identifiers(
        function_comma_shadowed_free,
        definitions,
        tla,
        current="Known",
        exactness_operator="IdentifierScanExactness",
    ) == ["FreeMissing"]


def test_transitive_undefined_exactness_conjuncts_preserve_operator_parameter_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedUndefinedEnvelope.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedUndefinedEnvelope ----",
                "ParamHelper(MissingHelper) == MissingHelper",
                "Root == ParamHelper",
                "ParameterizedExactness ==",
                "  /\\ Root",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    signatures = module.tla_operator_signatures(tla)
    assert module.transitive_undefined_exactness_conjuncts(
        "ParameterizedExactness",
        "/\\ Root",
        definitions,
        tla,
    ) == []
    assert module.transitive_nonzero_arity_exactness_conjuncts(
        "ParameterizedExactness",
        "/\\ Root",
        definitions,
        signatures,
        tla,
    ) == [f"Root reaches Root -> ParamHelper at {tla}:2 with arity 1"]


def test_transitive_undefined_quantified_exactness_preserves_operator_parameter_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedQuantifiedUndefinedEnvelope.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedQuantifiedUndefinedEnvelope ----",
                "CONSTANT Cases",
                "ParamHelper(MissingHelper) == \\A Case \\in Cases: MissingHelper",
                "Root == ParamHelper",
                "ParameterizedExactness ==",
                "  /\\ Root",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "ParameterizedExactness",
        "/\\ Root",
        definitions,
        tla,
    ) == []


def test_transitive_undefined_temporal_conjuncts_preserve_operator_parameter_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedTemporalUndefinedEnvelope.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedTemporalUndefinedEnvelope ----",
                "ParamTemporal(MissingTemporalPredicate) == MissingTemporalPredicate",
                "TemporalWrapper == ParamTemporal",
                "TemporalSafety == [] TemporalWrapper",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    signatures = module.tla_operator_signatures(tla)
    assert module.transitive_undefined_temporal_extra_conjuncts(
        "TemporalSafety",
        "[] TemporalWrapper",
        definitions,
        tla,
        4,
    ) == []
    assert module.nonzero_arity_temporal_helper_references(
        "TemporalSafety",
        "[] TemporalWrapper",
        signatures,
        definitions,
        tla,
    ) == [
        "TemporalWrapper reaches TemporalWrapper -> ParamTemporal at "
        f"{tla}:2 with arity 1"
    ]


def test_transitive_undefined_quantified_temporal_preserves_operator_parameter_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedQuantifiedTemporalUndefinedEnvelope.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedQuantifiedTemporalUndefinedEnvelope ----",
                "CONSTANT Cases",
                "ParamTemporal(MissingTemporalPredicate) == "
                "\\A Case \\in Cases: MissingTemporalPredicate",
                "TemporalWrapper == ParamTemporal",
                "TemporalSafety == [] TemporalWrapper",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_temporal_extra_conjuncts(
        "TemporalSafety",
        "[] TemporalWrapper",
        definitions,
        tla,
        5,
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_quantified_undefined_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiQuantifiedUndefinedEnvelope.tla"
    cfg = tmp_path / "SumeragiQuantifiedUndefinedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiQuantifiedUndefinedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "QuantifiedLeaf == \\A c \\in Cases: MissingHelper",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT QuantifiedUndefinedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "quantified-undefined-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        f"quantified-undefined-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope QuantifiedUndefinedCorrectnessEnvelope, but "
        f"exactness conjunct QuantifiedUndefinedExactness at {tla}:8 contains "
        "transitive exactness predicate chain with undefined quantified helper "
        f"QuantifiedLeaf reaches MissingHelper through QuantifiedLeaf at "
        f"{tla}:6; define named concrete model predicates before composing "
        "exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_shadowed_quantified_undefined_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiShadowedQuantifiedUndefinedEnvelope.tla"
    cfg = tmp_path / "SumeragiShadowedQuantifiedUndefinedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiShadowedQuantifiedUndefinedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ (\\A MissingHelper \\in Cases: ConcretePredicate)",
                "    /\\ MissingHelper",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT QuantifiedUndefinedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "shadowed-quantified-undefined-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        f"shadowed-quantified-undefined-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope QuantifiedUndefinedCorrectnessEnvelope, "
        "but exactness conjunct QuantifiedUndefinedExactness at "
        f"{tla}:12 contains transitive exactness predicate chain with "
        "undefined quantified helper QuantifiedLeaf reaches MissingHelper "
        f"through QuantifiedLeaf at {tla}:8; define named concrete model "
        "predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_shadowed_quantified_undefined_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLetShadowedQuantifiedUndefinedEnvelope.tla"
    cfg = tmp_path / "SumeragiLetShadowedQuantifiedUndefinedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLetShadowedQuantifiedUndefinedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    LET MissingHelper == ConcretePredicate IN",
                "      /\\ MissingHelper",
                "      /\\ FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT QuantifiedUndefinedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "let-shadowed-quantified-undefined-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        f"let-shadowed-quantified-undefined-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope QuantifiedUndefinedCorrectnessEnvelope, "
        "but exactness conjunct QuantifiedUndefinedExactness at "
        f"{tla}:13 contains transitive exactness predicate chain with "
        "undefined quantified helper QuantifiedLeaf reaches FreeMissing "
        f"through QuantifiedLeaf at {tla}:8; define named concrete model "
        "predicates before composing exactness predicate chains"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_parameterized_let_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedLetShadowedQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedLetShadowedQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    LET LocalHelper(MissingParam) == "
                "ConcretePredicate /\\ MissingParam IN",
                "      /\\ LocalHelper(Case)",
                "      /\\ FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_lambda_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLambdaShadowedQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLambdaShadowedQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    (LAMBDA MissingHelper \\in Cases:",
                "      ConcretePredicate /\\ MissingHelper /\\ FreeMissing)[Case]",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_set_comprehension_enclosure_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSetEnclosureQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSetEnclosureQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    {MissingHelper \\in Cases: ConcretePredicate /\\ MissingHelper} /\\",
                "    {OtherMissing \\in Cases: OtherMissing} /\\ FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_action_wrapper_operand_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiActionWrapperQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiActionWrapperQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ ENABLED (ConcreteRecord.MissingField)",
                "    /\\ UNCHANGED <<ConcreteRecord.MissingField, FreeMissing>>",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_case_branch_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiCaseBranchQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiCaseBranchQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    CASE ConcreteRecord.MissingField ->",
                "      [MissingField |-> ConcreteRecord.MissingField]",
                "    [] OTHER -> FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_relation_and_call_argument_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRelationCallQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRelationCallQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "ConcretePredicate(Value) == Value = Value",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ ConcretePredicate(ConcreteRecord.MissingField)",
                "    /\\ ConcreteRecord.MissingField = TRUE",
                "    /\\ FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:9"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_arithmetic_and_set_infix_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiInfixQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiInfixQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "OtherRecord == [Present |-> TRUE]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ ConcreteRecord.MissingField + OtherRecord.OtherField",
                "    /\\ ConcreteRecord.MissingField \\cup {FreeMissing}",
                "    /\\ Known(ConcreteRecord.MissingField + FreeMissing)",
                "Known(Value) == Value = Value",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:9"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_sequence_function_infix_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSequenceFunctionInfixQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSequenceFunctionInfixQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "OtherRecord == [Present |-> TRUE]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ ConcreteRecord.MissingField \\o <<FreeMissing>>",
                "    /\\ ConcreteRecord.MissingField @@ OtherRecord.OtherField",
                "    /\\ ConcreteRecord.MissingField :> FreeMissing",
                "    /\\ ConcreteRecord.MissingField \\subseteq OtherRecord.OtherField",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:9"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_relation_bearing_binding_prefix_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRelationBindingPrefixQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRelationBindingPrefixQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ CHOOSE MissingHelper \\subseteq Cases: MissingHelper",
                "    /\\ LAMBDA MissingHelper # Cases: MissingHelper",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches MissingHelper through QuantifiedLeaf at {tla}:7"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_set_literal_and_unary_set_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSetLiteralUnarySetQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSetLiteralUnarySetQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ {[MissingField |-> ConcreteRecord.MissingField]}",
                "    /\\ DOMAIN [OtherField |-> ConcreteRecord.MissingField]",
                "    /\\ SUBSET {MissingHelper \\in Cases: "
                "ConcreteRecord.MissingField /\\ MissingHelper}",
                "    /\\ UNION {[UnionField |-> FreeMissing]}",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_function_set_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiFunctionSetQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiFunctionSetQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteRecord == [Present |-> TRUE]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ [Cases -> [MissingField |-> ConcreteRecord.MissingField]]",
                "    /\\ [Cases -> [MissingHelper \\in Cases |-> MissingHelper]]",
                "    /\\ [Cases -> DOMAIN [OtherField |-> FreeMissing]]",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_tuple_pattern_domain_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTupleDomainQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTupleDomainQuantifiedUndefined ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A <<Case, OtherCase>> \\in MissingDomain:",
                "    ConcretePredicate /\\ Case /\\ OtherCase",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches MissingDomain through QuantifiedLeaf at {tla}:7"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_record_field_labels(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRecordFieldQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRecordFieldQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    [MissingField |-> ConcretePredicate, OtherField |-> FreeMissing]",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_record_set_and_update_field_labels(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRecordSetUpdateQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRecordSetUpdateQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "ConcreteRecord == [Field |-> ConcretePredicate]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    [MissingField: FreeMissing] /\\",
                "      [ConcreteRecord EXCEPT ![IndexMissing].OtherField = FreeMissingValue]",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:9",
        f"QuantifiedLeaf reaches FreeMissingValue through QuantifiedLeaf at {tla}:9",
        f"QuantifiedLeaf reaches IndexMissing through QuantifiedLeaf at {tla}:9",
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_record_selector_field_labels(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRecordSelectorQuantifiedUndefined.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRecordSelectorQuantifiedUndefined ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "ConcreteRecord == [Field |-> ConcretePredicate]",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    ConcreteRecord.MissingField /\\",
                "      ConcreteRecord[IndexMissing].OtherField /\\",
                "      ConcreteRecord.MissingField[ValueMissing]",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches IndexMissing through QuantifiedLeaf at {tla}:9",
        f"QuantifiedLeaf reaches ValueMissing through QuantifiedLeaf at {tla}:9",
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_choose_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiChooseShadowedQuantifiedUndefinedEnvelope.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiChooseShadowedQuantifiedUndefinedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ (CHOOSE MissingHelper \\in Cases: ConcretePredicate /\\ MissingHelper)",
                "    /\\ FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_transitive_undefined_quantified_exactness_conjuncts_preserves_set_and_function_scope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSetFunctionShadowedQuantifiedUndefinedEnvelope.tla"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSetFunctionShadowedQuantifiedUndefinedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "QuantifiedLeaf ==",
                "  \\A Case \\in Cases:",
                "    /\\ {MissingHelper \\in Cases: ConcretePredicate /\\ MissingHelper}",
                "    /\\ [FunctionMissing \\in Cases |-> ConcretePredicate /\\ FunctionMissing]",
                "    /\\ {MissingLeft, MissingRight \\in Cases: ConcretePredicate /\\ MissingLeft /\\ MissingRight}",
                "    /\\ [FunctionLeft, FunctionRight \\in Cases |-> ConcretePredicate /\\ FunctionLeft /\\ FunctionRight]",
                "    /\\ FreeMissing",
                "QuantifiedUndefinedExactness ==",
                "  /\\ QuantifiedLeaf",
                "QuantifiedUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ QuantifiedUndefinedExactness",
                "====",
            ]
        ),
        encoding="utf-8",
    )

    definitions = module.tla_single_expression_operator_definitions(tla)
    assert module.transitive_undefined_quantified_exactness_conjuncts(
        "QuantifiedUndefinedExactness",
        "/\\ QuantifiedLeaf",
        definitions,
        tla,
    ) == [
        f"QuantifiedLeaf reaches FreeMissing through QuantifiedLeaf at {tla}:8"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnaryTemporalQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiUnaryTemporalQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnaryTemporalQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "QuantifiedWrapper == [] (\\A c \\in Cases: Predicate(c))",
                "UnaryTemporalQuantifiedExactness ==",
                "  /\\ QuantifiedWrapper",
                "UnaryTemporalQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ UnaryTemporalQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnaryTemporalQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unary-temporal-quantified-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "unary-temporal-quantified-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "UnaryTemporalQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"UnaryTemporalQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with unary-temporal quantified formula "
        "QuantifiedWrapper reaches \\A c \\in Cases: Predicate(c) through "
        f"QuantifiedWrapper at {tla}:7; name quantified model predicates "
        "before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_static_wrapped_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStaticWrappedQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiStaticWrappedQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStaticWrappedQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "QuantifiedWrapper == SUBSET (\\A c \\in Cases: Predicate(c))",
                "StaticWrappedQuantifiedExactness ==",
                "  /\\ QuantifiedWrapper",
                "StaticWrappedQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StaticWrappedQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StaticWrappedQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "static-wrapped-quantified-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "static-wrapped-quantified-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StaticWrappedQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"StaticWrappedQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with static-wrapper quantified formula "
        "QuantifiedWrapper reaches SUBSET wraps "
        "\\A c \\in Cases: Predicate(c) through QuantifiedWrapper at "
        f"{tla}:7; name quantified model predicates before composing "
        "exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_structured_quantified_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredQuantifiedEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredQuantifiedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredQuantifiedEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Predicate(c) == checked = c",
                "QuantifiedWrapper == [Field |-> \\A c \\in Cases: Predicate(c)]",
                "StructuredQuantifiedExactness ==",
                "  /\\ QuantifiedWrapper",
                "StructuredQuantifiedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredQuantifiedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredQuantifiedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-quantified-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-quantified-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredQuantifiedCorrectnessEnvelope, but exactness conjunct "
        f"StructuredQuantifiedExactness at {tla}:9 contains transitive "
        "exactness predicate chain with structured quantified formula "
        "QuantifiedWrapper reaches record literal contains "
        "\\A c \\in Cases: Predicate(c) through QuantifiedWrapper at "
        f"{tla}:7; name quantified model predicates before placing them in "
        "structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_structured_control_flow_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteLeaf == checked = ready",
                "FallbackLeaf == checked = committed",
                "StructuredWrapper == [Field |-> IF Ready THEN ConcreteLeaf ELSE FallbackLeaf]",
                "StructuredControlFlowExactness ==",
                "  /\\ StructuredWrapper",
                "StructuredControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-control-flow-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-control-flow-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"StructuredControlFlowExactness at {tla}:9 contains transitive "
        "exactness predicate chain with structured control-flow "
        "predicate-selection helper StructuredWrapper reaches record literal "
        "contains IF expression IF Ready THEN ConcreteLeaf ELSE FallbackLeaf "
        f"through StructuredWrapper at {tla}:7; name concrete model "
        "predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_bare_case_record_control_flow_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredCaseControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredCaseControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredCaseControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteLeaf == checked = ready",
                "FallbackLeaf == checked = committed",
                "StructuredWrapper == [Field |-> CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf]",
                "StructuredControlFlowExactness ==",
                "  /\\ StructuredWrapper",
                "StructuredControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-case-control-flow-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-case-control-flow-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"StructuredControlFlowExactness at {tla}:9 contains transitive "
        "exactness predicate chain with structured control-flow "
        "predicate-selection helper StructuredWrapper reaches record literal "
        "contains CASE expression CASE Ready -> ConcreteLeaf [] OTHER -> "
        f"FallbackLeaf through StructuredWrapper at {tla}:7; name concrete "
        "model predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_bare_case_record_set_control_flow_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredCaseRecordSetEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredCaseRecordSetEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredCaseRecordSetEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteLeaf == checked = ready",
                "FallbackLeaf == checked = committed",
                "StructuredWrapper == [Field: CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf]",
                "StructuredControlFlowExactness ==",
                "  /\\ StructuredWrapper",
                "StructuredControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-case-record-set-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-case-record-set-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"StructuredControlFlowExactness at {tla}:9 contains transitive "
        "exactness predicate chain with structured control-flow "
        "predicate-selection helper StructuredWrapper reaches record set "
        "contains CASE expression CASE Ready -> ConcreteLeaf [] OTHER -> "
        f"FallbackLeaf through StructuredWrapper at {tla}:7; name concrete "
        "model predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_bare_case_record_update_control_flow_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredCaseRecordUpdateEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredCaseRecordUpdateEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredCaseRecordUpdateEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteLeaf == checked = ready",
                "FallbackLeaf == checked = committed",
                "record == [field |-> 0]",
                "StructuredWrapper == [record EXCEPT !.field = CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf]",
                "StructuredControlFlowExactness ==",
                "  /\\ StructuredWrapper",
                "StructuredControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-case-record-update-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-case-record-update-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"StructuredControlFlowExactness at {tla}:10 contains transitive "
        "exactness predicate chain with structured control-flow "
        "predicate-selection helper StructuredWrapper reaches record update "
        "contains CASE expression CASE Ready -> ConcreteLeaf [] OTHER -> "
        f"FallbackLeaf through StructuredWrapper at {tla}:8; name concrete "
        "model predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_bare_case_function_set_domain_control_flow_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredCaseFunctionSetDomainEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredCaseFunctionSetDomainEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredCaseFunctionSetDomainEnvelope ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteLeaf == checked = ready",
                "FallbackLeaf == checked = committed",
                "StructuredWrapper == [CASE Ready -> ConcreteLeaf [] OTHER -> FallbackLeaf -> Cases]",
                "StructuredControlFlowExactness ==",
                "  /\\ StructuredWrapper",
                "StructuredControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredControlFlowExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-case-function-set-domain-envelope-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-case-function-set-domain-envelope-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"StructuredControlFlowExactness at {tla}:10 contains transitive "
        "exactness predicate chain with structured control-flow "
        "predicate-selection helper StructuredWrapper reaches function set "
        "contains CASE expression CASE Ready -> ConcreteLeaf [] OTHER -> "
        f"FallbackLeaf through StructuredWrapper at {tla}:8; name concrete "
        "model predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_structured_literal_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStructuredLiteralEnvelope.tla"
    cfg = tmp_path / "SumeragiStructuredLiteralEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStructuredLiteralEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "LiteralLeaf == TRUE",
                "StructuredWrapper == [Field |-> LiteralLeaf]",
                "StructuredExactness ==",
                "  /\\ StructuredWrapper",
                "StructuredCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ StructuredExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT StructuredCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "structured-literal-fast",
        tla,
        cfg,
        "TLC",
    ) == [
        "structured-literal-fast: TLC cfg "
        f"{cfg}:4 references correctness envelope "
        "StructuredCorrectnessEnvelope, but exactness conjunct "
        f"StructuredExactness at {tla}:8 contains transitive exactness "
        "predicate chain with vacuous conjunct StructuredWrapper reaches "
        "LiteralLeaf through StructuredWrapper -> LiteralLeaf at "
        f"{tla}:5 is literal TRUE; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_vacuous_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveVacuousEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveVacuousEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveVacuousEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "LiteralLeaf == [] (TRUE /\\ TRUE)",
                "AliasLeaf == ModelPredicate",
                "LiteralWrapper ==",
                "  /\\ LiteralLeaf",
                "AliasWrapper ==",
                "  /\\ AliasLeaf",
                "TransitiveVacuousExactness ==",
                "  /\\ LiteralWrapper",
                "  /\\ AliasWrapper",
                "TransitiveVacuousCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveVacuousExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveVacuousCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-vacuous-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-vacuous-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveVacuousCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveVacuousExactness at {tla}:13 contains transitive "
        "exactness predicate chain with vacuous conjunct LiteralWrapper "
        "reaches LiteralWrapper through LiteralWrapper at "
        f"{tla}:9 aliases LiteralLeaf through a single helper conjunct, "
        "LiteralWrapper "
        f"reaches LiteralLeaf through LiteralWrapper -> LiteralLeaf at {tla}:6 "
        "is literal TRUE, AliasWrapper reaches AliasWrapper through "
        "AliasWrapper at "
        f"{tla}:11 aliases AliasLeaf through a single helper conjunct, "
        "AliasWrapper reaches AliasLeaf through AliasWrapper -> AliasLeaf at "
        f"{tla}:7 aliases ModelPredicate; keep "
        "literal, self-equality, self-inequality, and alias helpers out of "
        "named exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_static_if_literal_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveStaticIfLiteralEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveStaticIfLiteralEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveStaticIfLiteralEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "StaticIfLiteralLeaf == IF FALSE THEN TRUE ELSE FALSE",
                "StaticIfLiteralWrapper == ModelPredicate /\\ StaticIfLiteralLeaf",
                "TransitiveStaticIfLiteralExactness ==",
                "  /\\ StaticIfLiteralWrapper",
                "TransitiveStaticIfLiteralCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveStaticIfLiteralExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveStaticIfLiteralCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-static-if-literal-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-static-if-literal-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveStaticIfLiteralCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveStaticIfLiteralExactness at {tla}:9 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "StaticIfLiteralWrapper reaches StaticIfLiteralLeaf through "
        f"StaticIfLiteralWrapper -> StaticIfLiteralLeaf at {tla}:6 is static "
        "IF literal FALSE; keep literal, self-equality, self-inequality, and "
        "alias helpers out of named exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_constant_relation_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveConstantRelationEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveConstantRelationEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveConstantRelationEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "ConstantRelationLeaf == 1 \\in {1}",
                "ConstantRelationWrapper == ModelPredicate /\\ ConstantRelationLeaf",
                "TransitiveConstantRelationExactness ==",
                "  /\\ ConstantRelationWrapper",
                "TransitiveConstantRelationCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveConstantRelationExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveConstantRelationCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-constant-relation-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-constant-relation-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveConstantRelationCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveConstantRelationExactness at {tla}:9 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "ConstantRelationWrapper reaches ConstantRelationLeaf through "
        f"ConstantRelationWrapper -> ConstantRelationLeaf at {tla}:6 is "
        "constant relation 1 \\in {1}; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_unary_temporal_constant_relation_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveUnaryTemporalConstantRelationEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveUnaryTemporalConstantRelationEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveUnaryTemporalConstantRelationEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "ConstantRelationLeaf == [] (1 \\in {1})",
                "ConstantRelationWrapper == ModelPredicate /\\ ConstantRelationLeaf",
                "TransitiveConstantRelationExactness ==",
                "  /\\ ConstantRelationWrapper",
                "TransitiveConstantRelationCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveConstantRelationExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveConstantRelationCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-unary-temporal-constant-relation-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"transitive-unary-temporal-constant-relation-envelope-fast: Apalache "
        f"cfg {cfg}:4 references correctness envelope "
        "TransitiveConstantRelationCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveConstantRelationExactness at {tla}:9 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "ConstantRelationWrapper reaches ConstantRelationLeaf through "
        f"ConstantRelationWrapper -> ConstantRelationLeaf at {tla}:6 is "
        "constant relation [] (1 \\in {1}); keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_transitive_vacuous_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiCompoundTransitiveVacuousEnvelope.tla"
    cfg = tmp_path / "SumeragiCompoundTransitiveVacuousEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiCompoundTransitiveVacuousEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "LiteralLeaf == [] (TRUE /\\ TRUE)",
                "AliasLeaf == ModelPredicate",
                "CompoundLiteralWrapper == LiteralLeaf \\/ ModelPredicate",
                "NegatedAliasWrapper == ~AliasLeaf",
                "CompoundTransitiveVacuousExactness ==",
                "  /\\ CompoundLiteralWrapper",
                "  /\\ NegatedAliasWrapper",
                "CompoundTransitiveVacuousCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ CompoundTransitiveVacuousExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT CompoundTransitiveVacuousCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "compound-transitive-vacuous-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"compound-transitive-vacuous-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "CompoundTransitiveVacuousCorrectnessEnvelope, but exactness conjunct "
        f"CompoundTransitiveVacuousExactness at {tla}:11 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "CompoundLiteralWrapper reaches LiteralLeaf through "
        f"CompoundLiteralWrapper -> LiteralLeaf at {tla}:6 is literal TRUE, "
        "NegatedAliasWrapper reaches AliasLeaf through "
        f"NegatedAliasWrapper -> AliasLeaf at {tla}:7 aliases "
        "ModelPredicate; keep literal, self-equality, self-inequality, and "
        "alias helpers out of named exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_self_inequality_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveSelfInequalityEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveSelfInequalityEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveSelfInequalityEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SelfInequalityLeaf == [] (checked # checked)",
                "SelfInequalityWrapper == [] SelfInequalityLeaf",
                "TransitiveSelfInequalityExactness ==",
                "  /\\ SelfInequalityWrapper",
                "TransitiveSelfInequalityCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveSelfInequalityExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveSelfInequalityCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-self-inequality-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"transitive-self-inequality-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "TransitiveSelfInequalityCorrectnessEnvelope, but exactness conjunct "
        f"TransitiveSelfInequalityExactness at {tla}:8 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "SelfInequalityWrapper reaches SelfInequalityLeaf through "
        f"SelfInequalityWrapper -> SelfInequalityLeaf at {tla}:5 contains "
        "self-inequality checked # checked; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_single_conjunct_exactness_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSingleConjunctAliasEnvelope.tla"
    cfg = tmp_path / "SumeragiSingleConjunctAliasEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSingleConjunctAliasEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FormulaLeaf == checked = ready",
                "SingleConjunctWrapper ==",
                "  /\\ FormulaLeaf",
                "SingleConjunctAliasExactness ==",
                "  /\\ SingleConjunctWrapper",
                "SingleConjunctAliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SingleConjunctAliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SingleConjunctAliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "single-conjunct-alias-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"single-conjunct-alias-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope SingleConjunctAliasCorrectnessEnvelope, but "
        f"exactness conjunct SingleConjunctAliasExactness at {tla}:9 contains "
        "transitive exactness predicate chain with vacuous conjunct "
        "SingleConjunctWrapper reaches SingleConjunctWrapper through "
        f"SingleConjunctWrapper at {tla}:7 aliases FormulaLeaf through a "
        "single helper conjunct; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_single_conjunct_exactness_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnaryTemporalSingleConjunctAliasEnvelope.tla"
    cfg = tmp_path / "SumeragiUnaryTemporalSingleConjunctAliasEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnaryTemporalSingleConjunctAliasEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FormulaLeaf == checked = ready",
                "SingleConjunctWrapper == [] (/\\ FormulaLeaf)",
                "SingleConjunctAliasExactness ==",
                "  /\\ SingleConjunctWrapper",
                "SingleConjunctAliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SingleConjunctAliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SingleConjunctAliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unary-temporal-single-conjunct-alias-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"unary-temporal-single-conjunct-alias-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "SingleConjunctAliasCorrectnessEnvelope, but exactness conjunct "
        f"SingleConjunctAliasExactness at {tla}:8 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "SingleConjunctWrapper reaches SingleConjunctWrapper through "
        f"SingleConjunctWrapper at {tla}:6 aliases FormulaLeaf through a "
        "single helper conjunct; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_gated_single_conjunct_exactness_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLiteralGatedSingleConjunctAliasEnvelope.tla"
    cfg = tmp_path / "SumeragiLiteralGatedSingleConjunctAliasEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLiteralGatedSingleConjunctAliasEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FormulaLeaf == checked = ready",
                "SingleConjunctWrapper == TRUE => (/\\ FormulaLeaf)",
                "SingleConjunctAliasExactness ==",
                "  /\\ SingleConjunctWrapper",
                "SingleConjunctAliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SingleConjunctAliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SingleConjunctAliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "literal-gated-single-conjunct-alias-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"literal-gated-single-conjunct-alias-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "SingleConjunctAliasCorrectnessEnvelope, but exactness conjunct "
        f"SingleConjunctAliasExactness at {tla}:8 contains transitive "
        "exactness predicate chain with vacuous conjunct "
        "SingleConjunctWrapper reaches SingleConjunctWrapper through "
        f"SingleConjunctWrapper at {tla}:6 aliases FormulaLeaf through a "
        "single helper conjunct; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_gated_zero_arity_exactness_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLiteralGatedZeroArityAliasEnvelope.tla"
    cfg = tmp_path / "SumeragiLiteralGatedZeroArityAliasEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLiteralGatedZeroArityAliasEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FormulaLeaf == checked = ready",
                "ZeroArityAliasWrapper == TRUE /\\ FormulaLeaf",
                "ZeroArityAliasExactness ==",
                "  /\\ ZeroArityAliasWrapper",
                "ZeroArityAliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ZeroArityAliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ZeroArityAliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "literal-gated-zero-arity-alias-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"literal-gated-zero-arity-alias-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope ZeroArityAliasCorrectnessEnvelope, "
        f"but exactness conjunct ZeroArityAliasExactness at {tla}:8 contains "
        "transitive exactness predicate chain with vacuous conjunct "
        "ZeroArityAliasWrapper reaches ZeroArityAliasWrapper through "
        f"ZeroArityAliasWrapper at {tla}:6 aliases FormulaLeaf through a "
        "literal-gated helper operand; keep literal, self-equality, "
        "self-inequality, and alias helpers out of named exactness predicate "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_direct_formula_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectFormulaEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectFormulaEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectFormulaEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FormulaLeaf == checked = ready",
                "DirectFormulaExactness ==",
                "  /\\ FormulaLeaf",
                "DirectFormulaCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectFormulaExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectFormulaCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "direct-formula-envelope-fast", tla, cfg, "TLC"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_self_equality_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiSelfEqualityEnvelope.tla"
    cfg = tmp_path / "SumeragiSelfEqualityEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiSelfEqualityEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SelfEqualityLeaf == checked = checked",
                "SelfEqualityExactness ==",
                "  /\\ SelfEqualityLeaf",
                "SelfEqualityCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SelfEqualityExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SelfEqualityCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "self-equality-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"self-equality-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope SelfEqualityCorrectnessEnvelope, but exactness "
        f"conjunct SelfEqualityExactness at {tla}:7 contains self-equality "
        f"exactness conjunct SelfEqualityLeaf at {tla}:5 is self-equality "
        "checked = checked; compose concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_self_inequality_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "SumeragiSelfInequalityEnvelope_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "INVARIANT SelfInequalityCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for case_name, formula in (
        ("hash", "checked # checked"),
        ("slash", "checked /= checked"),
    ):
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "SumeragiSelfInequalityEnvelope.tla"
        local_cfg = case_dir / "SumeragiSelfInequalityEnvelope_fast.cfg"
        local_cfg.write_text(cfg.read_text(encoding="utf-8"), encoding="utf-8")
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE SumeragiSelfInequalityEnvelope ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    f"SelfInequalityLeaf == {formula}",
                    "SelfInequalityExactness ==",
                    "  /\\ SelfInequalityLeaf",
                    "SelfInequalityCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SelfInequalityExactness",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "self-inequality-envelope-fast", tla, local_cfg, "Apalache"
        ) == [
            "self-inequality-envelope-fast: Apalache cfg "
            f"{local_cfg}:4 references correctness envelope "
            "SelfInequalityCorrectnessEnvelope, but exactness conjunct "
            f"SelfInequalityExactness at {tla}:7 contains self-inequality "
            f"exactness conjunct SelfInequalityLeaf at {tla}:5 is "
            f"self-inequality {formula}; compose satisfiable concrete model "
            "predicates directly"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_self_equality_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTemporalSelfEqualityEnvelope.tla"
    cfg = tmp_path / "SumeragiTemporalSelfEqualityEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTemporalSelfEqualityEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SelfEqualityLeaf == [] (checked = checked)",
                "SelfEqualityExactness ==",
                "  /\\ SelfEqualityLeaf",
                "SelfEqualityCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SelfEqualityExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SelfEqualityCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "temporal-self-equality-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"temporal-self-equality-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope SelfEqualityCorrectnessEnvelope, but "
        f"exactness conjunct SelfEqualityExactness at {tla}:7 contains "
        f"self-equality exactness conjunct SelfEqualityLeaf at {tla}:5 "
        "contains self-equality checked = checked; compose concrete model "
        "predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_undefined_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUndefinedConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiUndefinedConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUndefinedConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "KnownPredicate == checked = ready",
                "MissingConjunctExactness ==",
                "  /\\ KnownPredicate",
                "  /\\ MissingPredicate",
                "MissingConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ MissingConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT MissingConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "undefined-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"undefined-conjunct-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope MissingConjunctCorrectnessEnvelope, but "
        f"exactness conjunct MissingConjunctExactness at {tla}:7 contains "
        "undefined exactness conjunct MissingPredicate; define named concrete "
        "model predicates before composing them"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_undefined_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTransitiveUndefinedEnvelope.tla"
    cfg = tmp_path / "SumeragiTransitiveUndefinedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTransitiveUndefinedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Wrapper == TRUE /\\ MissingPredicate",
                "TransitiveUndefinedExactness ==",
                "  /\\ Wrapper",
                "TransitiveUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TransitiveUndefinedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TransitiveUndefinedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "transitive-undefined-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"transitive-undefined-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope TransitiveUndefinedCorrectnessEnvelope, but "
        f"exactness conjunct TransitiveUndefinedExactness at {tla}:7 contains "
        "transitive exactness predicate chain with undefined conjunct "
        f"Wrapper reaches MissingPredicate through Wrapper at {tla}:5; "
        "define named concrete model predicates before composing exactness "
        "predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_transitive_undefined_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiCompoundTransitiveUndefinedEnvelope.tla"
    cfg = tmp_path / "SumeragiCompoundTransitiveUndefinedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiCompoundTransitiveUndefinedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "Wrapper == MissingPredicate \\/ ModelPredicate",
                "CompoundTransitiveUndefinedExactness ==",
                "  /\\ Wrapper",
                "CompoundTransitiveUndefinedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ CompoundTransitiveUndefinedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT CompoundTransitiveUndefinedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "compound-transitive-undefined-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"compound-transitive-undefined-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "CompoundTransitiveUndefinedCorrectnessEnvelope, but exactness conjunct "
        f"CompoundTransitiveUndefinedExactness at {tla}:8 contains transitive "
        "exactness predicate chain with undefined conjunct Wrapper reaches "
        f"MissingPredicate through Wrapper at {tla}:6; define named concrete "
        "model predicates before composing exactness predicate chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_lowercase_state_variable_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiLowercaseStateHelperEnvelope.tla"
    cfg = tmp_path / "SumeragiLowercaseStateHelperEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiLowercaseStateHelperEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "Wrapper == checked /\\ ModelPredicate",
                "LowercaseStateExactness ==",
                "  /\\ Wrapper",
                "LowercaseStateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ LowercaseStateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT LowercaseStateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "lowercase-state-helper-envelope-fast", tla, cfg, "Apalache"
    ) == []


def test_cfg_correctness_envelope_shape_errors_allows_declared_constant_exactness_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDeclaredConstantHelperEnvelope.tla"
    cfg = tmp_path / "SumeragiDeclaredConstantHelperEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDeclaredConstantHelperEnvelope ----",
                "CONSTANTS BugFlag",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "Wrapper == BugFlag /\\ ModelPredicate",
                "DeclaredConstantExactness ==",
                "  /\\ Wrapper",
                "DeclaredConstantCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DeclaredConstantExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DeclaredConstantCorrectnessEnvelope",
                "CONSTANTS",
                "  BugFlag = TRUE",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "declared-constant-helper-envelope-fast", tla, cfg, "TLC"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_raw_scalar_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiRawScalarEnvelope.tla"
    cfg = tmp_path / "SumeragiRawScalarEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiRawScalarEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ActualOutput == 1",
                "SpecOutput == 1",
                "RawScalarExactness ==",
                "  ActualOutput = SpecOutput",
                "RawScalarCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ RawScalarExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT RawScalarCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "raw-scalar-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"raw-scalar-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope RawScalarCorrectnessEnvelope, but exactness "
        f"conjunct RawScalarExactness at {tla}:8 is raw scalar equality "
        "ActualOutput = SpecOutput; name the concrete model predicate and "
        "compose it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_raw_equality_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectRawEqualityEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectRawEqualityEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectRawEqualityEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ActualOutput == 1",
                "SpecOutput == 1",
                "NamedPredicate == TRUE",
                "DirectRawEqualityConjunctExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ ActualOutput = SpecOutput",
                "DirectRawEqualityConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectRawEqualityConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectRawEqualityConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-raw-equality-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"direct-raw-equality-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope DirectRawEqualityConjunctCorrectnessEnvelope, but "
        f"exactness conjunct DirectRawEqualityConjunctExactness at {tla}:9 "
        "contains direct raw scalar equality conjunct ActualOutput = SpecOutput; "
        "name the concrete model predicate and compose it as a direct exactness "
        "conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_negated_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNegatedEnvelope.tla"
    cfg = tmp_path / "SumeragiNegatedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNegatedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ViolatesContract == FALSE",
                "NegatedExactness ==",
                "  ~ViolatesContract",
                "NegatedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NegatedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NegatedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "negated-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"negated-envelope-fast: TLC cfg {cfg}:4 references correctness "
        "envelope NegatedCorrectnessEnvelope, but exactness conjunct "
        f"NegatedExactness at {tla}:7 is whole-body negation "
        "~ViolatesContract; name the concrete model predicate and compose it "
        "as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_disjunctive_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDisjunctiveEnvelope.tla"
    cfg = tmp_path / "SumeragiDisjunctiveEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDisjunctiveEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "NoneCase == \"none\"",
                "Candidate == \"none\"",
                "FieldsMatch == TRUE",
                "DisjunctiveExactness ==",
                "  Candidate = NoneCase \\/ FieldsMatch",
                "DisjunctiveCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DisjunctiveExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DisjunctiveCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "disjunctive-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"disjunctive-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope DisjunctiveCorrectnessEnvelope, but exactness "
        f"conjunct DisjunctiveExactness at {tla}:9 is whole-body disjunction "
        "Candidate = NoneCase \\/ FieldsMatch; name the concrete model "
        "predicate and compose it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_equivalence_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiEquivalenceEnvelope.tla"
    cfg = tmp_path / "SumeragiEquivalenceEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiEquivalenceEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == TRUE",
                "OtherPredicate == TRUE",
                "EquivalenceExactness ==",
                "  ModelPredicate <=> OtherPredicate",
                "EquivalenceCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ EquivalenceExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT EquivalenceCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "equivalence-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"equivalence-envelope-fast: TLC cfg {cfg}:4 references correctness "
        "envelope EquivalenceCorrectnessEnvelope, but exactness conjunct "
        f"EquivalenceExactness at {tla}:8 is whole-body equivalence "
        "ModelPredicate <=> OtherPredicate; name the concrete model predicate "
        "and compose it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_control_flow_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiControlFlowEnvelope.tla"
    cfg = tmp_path / "SumeragiControlFlowEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiControlFlowEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == TRUE",
                "OtherPredicate == TRUE",
                "CaseExactness ==",
                "  CASE TRUE -> ModelPredicate [] OTHER -> OtherPredicate",
                "IfExactness ==",
                "  IF TRUE THEN ModelPredicate ELSE OtherPredicate",
                "ImplicationExactness ==",
                "  ModelPredicate => OtherPredicate",
                "ControlFlowCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ IfExactness",
                "  /\\ CaseExactness",
                "  /\\ ImplicationExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ControlFlowCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "control-flow-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"control-flow-envelope-fast: TLC cfg {cfg}:4 references correctness "
        "envelope ControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"CaseExactness at {tla}:8 is whole-body CASE expression CASE TRUE -> "
        "ModelPredicate [] OTHER -> OtherPredicate; name the concrete model "
        "predicate and compose it as a direct exactness conjunct",
        f"control-flow-envelope-fast: TLC cfg {cfg}:4 references correctness "
        "envelope ControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"IfExactness at {tla}:10 is whole-body IF expression IF TRUE THEN "
        "ModelPredicate ELSE OtherPredicate; name the concrete model predicate "
        "and compose it as a direct exactness conjunct",
        f"control-flow-envelope-fast: TLC cfg {cfg}:4 references correctness "
        "envelope ControlFlowCorrectnessEnvelope, but exactness conjunct "
        f"ImplicationExactness at {tla}:12 is whole-body implication "
        "ModelPredicate => OtherPredicate; name the concrete model predicate "
        "and compose it as a direct exactness conjunct",
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_action_quantifier_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiActionQuantifierEnvelope.tla"
    cfg = tmp_path / "SumeragiActionQuantifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiActionQuantifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "ImplementationActions(c) == {}",
                "SpecActions(c) == {}",
                "ActionQuantifierExactness ==",
                "  \\A c \\in Cases: ImplementationActions(c) = SpecActions(c)",
                "ActionQuantifierCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ActionQuantifierExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ActionQuantifierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "action-quantifier-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"action-quantifier-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope ActionQuantifierCorrectnessEnvelope, but "
        f"exactness conjunct ActionQuantifierExactness at {tla}:9 is "
        "whole-body implementation/spec action quantifier "
        "\\A c \\in Cases: ImplementationActions(c) = SpecActions(c); name "
        "the concrete model predicate and compose it as a direct exactness "
        "conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_action_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectActionConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectActionConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectActionConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ImplementationActions(ResetEmpty) == {}",
                "SpecActions(ResetEmpty) == {}",
                "NamedActionMatch == ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)",
                "DirectActionConjunctExactness ==",
                "  /\\ NamedActionMatch",
                "  /\\ ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)",
                "DirectActionConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectActionConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectActionConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-action-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"direct-action-conjunct-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope DirectActionConjunctCorrectnessEnvelope, but "
        f"exactness conjunct DirectActionConjunctExactness at {tla}:9 "
        "contains direct implementation/spec action conjunct "
        "ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty); name "
        "the concrete model predicate and compose it as a direct exactness "
        "conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_action_quantifier_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectActionQuantifierEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectActionQuantifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectActionQuantifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Candidates == {\"empty\"}",
                "ImplementationActions(c) == {}",
                "SpecActions(c) == {}",
                "NamedPredicate == TRUE",
                "DirectActionQuantifierConjunctExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ \\A c \\in Candidates: ImplementationActions(c) = SpecActions(c)",
                "DirectActionQuantifierConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectActionQuantifierConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectActionQuantifierConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-action-quantifier-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"direct-action-quantifier-conjunct-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "DirectActionQuantifierConjunctCorrectnessEnvelope, but exactness "
        f"conjunct DirectActionQuantifierConjunctExactness at {tla}:10 contains "
        "direct implementation/spec action quantifier conjunct \\A c \\in "
        "Candidates: ImplementationActions(c) = SpecActions(c); name the "
        "concrete model predicate and compose it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_matches_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectMatchesConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectMatchesConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectMatchesConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Case == \"empty\"",
                "Matches(c) == TRUE",
                "NamedMatches == Matches(Case)",
                "DirectMatchesConjunctExactness ==",
                "  /\\ NamedMatches",
                "  /\\ Matches(Case)",
                "DirectMatchesConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectMatchesConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectMatchesConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-matches-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"direct-matches-conjunct-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "DirectMatchesConjunctCorrectnessEnvelope, but exactness conjunct "
        f"DirectMatchesConjunctExactness at {tla}:9 contains direct Matches "
        "conjunct Matches(Case); name the concrete model predicate and "
        "compose it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_matches_quantifier_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectMatchesQuantifierEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectMatchesQuantifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectMatchesQuantifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "Matches(c) == TRUE",
                "NamedPredicate == TRUE",
                "DirectMatchesQuantifierConjunctExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ \\A c \\in Cases: Matches(c)",
                "DirectMatchesQuantifierConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectMatchesQuantifierConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectMatchesQuantifierConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-matches-quantifier-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"direct-matches-quantifier-conjunct-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "DirectMatchesQuantifierConjunctCorrectnessEnvelope, but exactness "
        f"conjunct DirectMatchesQuantifierConjunctExactness at {tla}:9 contains "
        "direct Matches quantifier conjunct \\A c \\in Cases: Matches(c); name "
        "the concrete matches predicate and compose it as a direct exactness "
        "conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_call_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedCallEnvelope.tla"
    cfg = tmp_path / "SumeragiParameterizedCallEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedCallEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "NamedPredicate == TRUE",
                "ParameterizedPredicate(c) == TRUE",
                "ParameterizedCallExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ ParameterizedPredicate(ready = checked)",
                "ParameterizedCallCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParameterizedCallExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParameterizedCallCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "parameterized-call-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"parameterized-call-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope ParameterizedCallCorrectnessEnvelope, but "
        f"exactness conjunct ParameterizedCallExactness at {tla}:9 contains "
        "direct parameterized exactness conjunct "
        "ParameterizedPredicate(ready = checked); lift the predicate behind a "
        "zero-arity model predicate before exactness composition"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_quantifier_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectQuantifierEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectQuantifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectQuantifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "ActualResult(c) == TRUE",
                "SpecResult(c) == TRUE",
                "NamedPredicate == TRUE",
                "DirectQuantifierConjunctExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ \\A c \\in Cases: ActualResult(c) = SpecResult(c)",
                "DirectQuantifierConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectQuantifierConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectQuantifierConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-quantifier-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"direct-quantifier-conjunct-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope DirectQuantifierConjunctCorrectnessEnvelope, but "
        f"exactness conjunct DirectQuantifierConjunctExactness at {tla}:10 "
        "contains direct quantifier conjunct \\A c \\in Cases: ActualResult(c) "
        "= SpecResult(c); name the concrete model predicate and compose it as a "
        "direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_formula_equality_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectFormulaEqualityEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectFormulaEqualityEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectFormulaEqualityEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ActualResult(c) == TRUE",
                "SpecResult(c) == TRUE",
                "NamedPredicate == TRUE",
                "DirectFormulaEqualityConjunctExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ ActualResult(\"empty\") = SpecResult(\"empty\")",
                "DirectFormulaEqualityConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectFormulaEqualityConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectFormulaEqualityConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-formula-equality-conjunct-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"direct-formula-equality-conjunct-envelope-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        "DirectFormulaEqualityConjunctCorrectnessEnvelope, but exactness "
        f"conjunct DirectFormulaEqualityConjunctExactness at {tla}:9 contains "
        "direct formula equality conjunct ActualResult(\"empty\") = "
        "SpecResult(\"empty\"); name the concrete model predicate and compose "
        "it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_direct_formula_equivalence_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectFormulaEquivalenceEnvelope.tla"
    cfg = tmp_path / "SumeragiDirectFormulaEquivalenceEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectFormulaEquivalenceEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ActualResult(c) == TRUE",
                "SpecResult(c) == TRUE",
                "NamedPredicate == TRUE",
                "DirectFormulaEquivalenceConjunctExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ ActualResult(\"empty\") <=> SpecResult(\"empty\")",
                "DirectFormulaEquivalenceConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectFormulaEquivalenceConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectFormulaEquivalenceConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "direct-formula-equivalence-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"direct-formula-equivalence-conjunct-envelope-fast: TLC cfg {cfg}:4 "
        "references correctness envelope "
        "DirectFormulaEquivalenceConjunctCorrectnessEnvelope, but exactness "
        f"conjunct DirectFormulaEquivalenceConjunctExactness at {tla}:9 "
        "contains direct formula equivalence conjunct ActualResult(\"empty\") "
        "<=> SpecResult(\"empty\"); name the concrete model predicate and "
        "compose it as a direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unnamed_conjunct_bundle(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnnamedConjunctEnvelope.tla"
    cfg = tmp_path / "SumeragiUnnamedConjunctEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnnamedConjunctEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "ActualResult(c) == TRUE",
                "SpecResult(c) == TRUE",
                "UnnamedConjunctExactness ==",
                "  /\\ TRUE",
                "UnnamedConjunctCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ UnnamedConjunctExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnnamedConjunctCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "unnamed-conjunct-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"unnamed-conjunct-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope UnnamedConjunctCorrectnessEnvelope, but "
        f"exactness conjunct UnnamedConjunctExactness at {tla}:9 is literal "
        "TRUE"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_mixed_non_named_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiMixedNonNamedEnvelope.tla"
    cfg = tmp_path / "SumeragiMixedNonNamedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiMixedNonNamedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "NamedPredicate == checked = ready",
                "MixedNonNamedExactness ==",
                "  /\\ NamedPredicate",
                "  /\\ TRUE",
                "MixedNonNamedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ MixedNonNamedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT MixedNonNamedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "mixed-non-named-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"mixed-non-named-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope MixedNonNamedCorrectnessEnvelope, but exactness "
        f"conjunct MixedNonNamedExactness at {tla}:7 contains direct non-named "
        "exactness conjunct TRUE; compose named zero-arity model predicates "
        "directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_matches_quantifier_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiMatchesQuantifierEnvelope.tla"
    cfg = tmp_path / "SumeragiMatchesQuantifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiMatchesQuantifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "Matches(c) == TRUE",
                "MatchesQuantifierExactness ==",
                "  \\A c \\in Cases: Matches(c)",
                "MatchesQuantifierCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ MatchesQuantifierExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT MatchesQuantifierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "matches-quantifier-envelope-fast", tla, cfg, "TLC"
    ) == [
        f"matches-quantifier-envelope-fast: TLC cfg {cfg}:4 references "
        "correctness envelope MatchesQuantifierCorrectnessEnvelope, but "
        f"exactness conjunct MatchesQuantifierExactness at {tla}:8 is "
        "whole-body Matches quantifier \\A c \\in Cases: Matches(c); name "
        "the concrete model predicate and compose it as a direct exactness "
        "conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_whole_body_quantifier_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiWholeQuantifierEnvelope.tla"
    cfg = tmp_path / "SumeragiWholeQuantifierEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiWholeQuantifierEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {\"empty\"}",
                "ActualResult(c) == TRUE",
                "SpecResult(c) == TRUE",
                "WholeQuantifierExactness ==",
                "  \\A c \\in Cases: ActualResult(c) = SpecResult(c)",
                "WholeQuantifierCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ WholeQuantifierExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT WholeQuantifierCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "whole-quantifier-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"whole-quantifier-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope WholeQuantifierCorrectnessEnvelope, but "
        f"exactness conjunct WholeQuantifierExactness at {tla}:9 is "
        "whole-body quantifier \\A c \\in Cases: ActualResult(c) = "
        "SpecResult(c); name the concrete model predicate and compose it as a "
        "direct exactness conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "ChildExactness ==",
                "  /\\ ConcretePredicate",
                "ParentExactness ==",
                "  /\\ ChildExactness",
                "ParentCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParentExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParentCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-exactness-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"nested-exactness-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope ParentCorrectnessEnvelope, but exactness "
        f"conjunct ParentExactness at {tla}:9 composes nested exactness "
        "ChildExactness; inline concrete model predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_type_invariant_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiTypeExactnessEnvelope.tla"
    cfg = tmp_path / "SumeragiTypeExactnessEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiTypeExactnessEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "TypeMixedExactness ==",
                "  /\\ TypeInvariant",
                "  /\\ ModelPredicate",
                "TypeMixedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ TypeMixedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT TypeMixedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "type-exactness-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"type-exactness-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope TypeMixedCorrectnessEnvelope, but exactness "
        f"conjunct TypeMixedExactness at {tla}:7 mentions TypeInvariant; keep "
        "type invariants in *CorrectnessEnvelope operators"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_type_and_exactness_mentions(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNestedEnvelope.tla"
    cfg = tmp_path / "SumeragiNestedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNestedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "NestedExactness ==",
                "  /\\ ModelPredicate",
                "NestedCorrectnessEnvelope ==",
                "  TypeInvariant => NestedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NestedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "nested-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"nested-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope NestedCorrectnessEnvelope, but "
        f"{tla}:9 contains direct non-named correctness-envelope conjunct "
        "TypeInvariant => NestedExactness; compose named zero-arity envelope "
        "predicates directly",
        f"nested-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope NestedCorrectnessEnvelope, but "
        f"{tla}:9 mentions TypeInvariant outside a top-level conjunct; "
        "compose TypeInvariant as a direct /\\ conjunct",
        f"nested-envelope-fast: Apalache cfg {cfg}:4 references "
        "correctness envelope NestedCorrectnessEnvelope, but "
        f"{tla}:9 mentions exactness NestedExactness outside top-level "
        "conjuncts; compose *Exactness operators as direct /\\ conjuncts",
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_duplicate_envelope_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDuplicateEnvelopeConjunct.tla"
    cfg = tmp_path / "SumeragiDuplicateEnvelopeConjunct_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDuplicateEnvelopeConjunct ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "DuplicateExactness ==",
                "  /\\ ModelPredicate",
                "DuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DuplicateExactness",
                "  /\\ DuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "duplicate-envelope-conjunct-fast", tla, cfg, "TLC"
    ) == [
        f"duplicate-envelope-conjunct-fast: TLC cfg {cfg}:4 references "
        "correctness envelope DuplicateCorrectnessEnvelope, but "
        f"{tla}:9 repeats correctness-envelope conjunct DuplicateExactness; "
        "remove duplicate conjuncts so every obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_mixed_non_named_envelope_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiNonNamedEnvelopeConjunct.tla"
    cfg = tmp_path / "SumeragiNonNamedEnvelopeConjunct_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiNonNamedEnvelopeConjunct ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "NonNamedExactness ==",
                "  /\\ ModelPredicate",
                "NonNamedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ NonNamedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT NonNamedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "non-named-envelope-conjunct-fast", tla, cfg, "TLC"
    ) == [
        f"non-named-envelope-conjunct-fast: TLC cfg {cfg}:4 references "
        "correctness envelope NonNamedCorrectnessEnvelope, but "
        f"{tla}:9 contains direct non-named correctness-envelope conjunct "
        "TRUE; compose named zero-arity envelope predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_envelope_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedEnvelopeConjunct.tla"
    cfg = tmp_path / "SumeragiParameterizedEnvelopeConjunct_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedEnvelopeConjunct ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant(c) == TRUE",
                "ModelPredicate == checked = ready",
                "ParameterizedEnvelopeExactness ==",
                "  /\\ ModelPredicate",
                "ParameterizedEnvelopeCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParameterizedEnvelopeExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParameterizedEnvelopeCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "parameterized-envelope-conjunct-fast", tla, cfg, "Apalache"
    ) == [
        f"parameterized-envelope-conjunct-fast: Apalache cfg {cfg}:4 "
        "references correctness envelope "
        f"ParameterizedEnvelopeCorrectnessEnvelope, but {tla}:9 contains "
        f"non-zero-arity correctness-envelope conjunct TypeInvariant at {tla}:4 "
        "has arity 1; correctness envelopes must compose zero-arity predicates "
        "directly"
    ]


def test_cfg_correctness_envelope_shape_errors_accepts_parenthesized_direct_conjuncts(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParenthesizedEnvelope.tla"
    cfg = tmp_path / "SumeragiParenthesizedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParenthesizedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "ParenthesizedExactness ==",
                "  /\\ ModelPredicate",
                "ParenthesizedCorrectnessEnvelope ==",
                "  /\\ (TypeInvariant)",
                "  /\\ (ParenthesizedExactness)",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParenthesizedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "parenthesized-envelope-fast", tla, cfg, "Apalache"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_generic_envelope_body(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiMixedEnvelope.tla"
    cfg = tmp_path / "SumeragiMixedEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiMixedEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "SafetyFast == ModelPredicate",
                "EnvelopeExactness ==",
                "  /\\ ModelPredicate",
                "MixedEnvelopeCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SafetyFast",
                "  /\\ EnvelopeExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT MixedEnvelopeCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "mixed-envelope-fast", tla, cfg, "Apalache"
    ) == [
        f"mixed-envelope-fast: Apalache cfg {cfg}:4 references correctness "
        f"envelope MixedEnvelopeCorrectnessEnvelope, but {tla}:10 mentions "
        "generic SafetyFast; compose concrete exactness predicates directly"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_non_exactness_body(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiMixedAnchorEnvelope.tla"
    cfg = tmp_path / "SumeragiMixedAnchorEnvelope_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiMixedAnchorEnvelope ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "EnvelopeExactness ==",
                "  /\\ ModelPredicate",
                "ModelPredicate == checked = ready",
                "ExtraAnchor == ModelPredicate",
                "MixedAnchorCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ EnvelopeExactness",
                "  /\\ ExtraAnchor",
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
                "INVARIANT TypeInvariant",
                "INVARIANT MixedAnchorCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "mixed-anchor-fast", tla, cfg, "TLC"
    ) == [
        f"mixed-anchor-fast: TLC cfg {cfg}:4 references correctness envelope "
        f"MixedAnchorCorrectnessEnvelope, but {tla}:10 mentions non-exactness "
        "ExtraAnchor; compose semantic obligations through *Exactness operators"
    ]


def test_cfg_correctness_envelope_shape_errors_accepts_top_level_temporal_extras(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_string_only_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                'EventuallyCommit == "ConcreteTemporalPredicate"',
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 has no "
        "static model identifiers; temporal correctness-envelope exceptions "
        "must name concrete model obligations"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_self_equality_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for case_name, eventually_commit_body, expected_line in (
        ("bare", "checked = checked", 9),
        ("unary", "[] (checked = checked)", 9),
    ):
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "Sumeragi.tla"
        local_cfg = case_dir / "Sumeragi_tlc_fast.cfg"
        local_cfg.write_text(cfg.read_text(encoding="utf-8"), encoding="utf-8")
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                    f"EventuallyCommit == {eventually_commit_body}",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, local_cfg, "TLC"
        ) == [
            f"fast: TLC cfg {local_cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            f"allowlisted temporal conjunct EventuallyCommit at {tla}:"
            f"{expected_line} contains self-equality checked = checked; "
            "temporal correctness-envelope exceptions must stay nontrivial"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_self_inequality_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for case_name, eventually_commit_body, expected_line in (
        ("bare", "checked # checked", 9),
        ("slash", "checked /= checked", 9),
        ("unary", "[] (checked # checked)", 9),
    ):
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "Sumeragi.tla"
        local_cfg = case_dir / "Sumeragi_tlc_fast.cfg"
        local_cfg.write_text(cfg.read_text(encoding="utf-8"), encoding="utf-8")
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                    f"EventuallyCommit == {eventually_commit_body}",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
                    "====",
                ]
            ),
            encoding="utf-8",
        )
        expected_inequality = (
            "checked /= checked" if case_name == "slash" else "checked # checked"
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, local_cfg, "TLC"
        ) == [
            f"fast: TLC cfg {local_cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            f"allowlisted temporal conjunct EventuallyCommit at {tla}:"
            f"{expected_line} contains self-inequality {expected_inequality}; "
            "temporal correctness-envelope exceptions must stay satisfiable"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == IF TRUE THEN [] (Gst => <> Committed) ELSE [] (Gst => <> Recovered)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "whole-body IF expression IF TRUE THEN [] (Gst => <> Committed) ELSE "
        "[] (Gst => <> Recovered); name the concrete temporal predicate before "
        "composing it as an allowlisted temporal side conjunct"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_boolean_composition_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cases = (
        ("negation", "~TemporalCommitPredicate"),
        ("disjunction", "TemporalCommitPredicate \\/ TemporalRecoveryPredicate"),
        ("implication", "TemporalCommitPredicate => TemporalRecoveryPredicate"),
        ("equivalence", "TemporalCommitPredicate <=> TemporalRecoveryPredicate"),
    )
    for operator_name, temporal_body in cases:
        case_dir = tmp_path / operator_name
        case_dir.mkdir()
        tla = case_dir / "Sumeragi.tla"
        cfg = case_dir / "Sumeragi_tlc_fast.cfg"
        cfg.write_text(
            "\n".join(
                [
                    "INIT Init",
                    "NEXT Next",
                    "INVARIANT TypeInvariant",
                    "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
                ]
            ),
            encoding="utf-8",
        )
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    "TemporalCommitPredicate == [] (Gst => <> Committed)",
                    "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                    f"EventuallyCommit == {temporal_body}",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, cfg, "TLC"
        ) == [
            f"fast: TLC cfg {cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            f"allowlisted temporal conjunct EventuallyCommit at {tla}:11 is "
            f"whole-body {operator_name} {temporal_body}; name the concrete "
            "temporal predicate before composing it as an allowlisted temporal "
            "side conjunct"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_helper_boolean_composition(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalCommitPredicate == [] (gst => <> committed)",
                "TemporalRecoveryPredicate == [] (gst => <> recovered)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (TemporalCommitPredicate => TemporalRecoveryPredicate)",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains temporal-helper boolean composition implication "
        "TemporalCommitPredicate => TemporalRecoveryPredicate; name concrete "
        "temporal predicates before composing allowlisted temporal "
        "side-conjunct chains"
    ]


def test_temporal_helper_boolean_composition_parts_descend_into_boolean_operands() -> None:
    module = load_coverage_module()

    definitions = {
        "ConcreteTemporal": (1, "[] (gst => <> done)"),
        "TemporalCommitPredicate": (2, "[] (gst => <> committed)"),
        "TemporalRecoveryPredicate": (3, "[] (gst => <> recovered)"),
    }

    assert module.temporal_helper_boolean_composition_parts(
        "ConcreteTemporal /\\ (TemporalCommitPredicate \\/ TemporalRecoveryPredicate)",
        definitions,
        include_nested_negation=False,
    ) == ["disjunction TemporalCommitPredicate \\/ TemporalRecoveryPredicate"]
    assert module.temporal_helper_boolean_composition_parts(
        "ConcreteTemporal /\\ (TemporalCommitPredicate => TemporalRecoveryPredicate)",
        definitions,
        include_nested_negation=False,
    ) == ["implication TemporalCommitPredicate => TemporalRecoveryPredicate"]
    assert module.temporal_helper_boolean_composition_parts(
        "ConcreteTemporal /\\ (TemporalCommitPredicate <=> TemporalRecoveryPredicate)",
        definitions,
        include_nested_negation=False,
    ) == ["equivalence TemporalCommitPredicate <=> TemporalRecoveryPredicate"]
    assert module.temporal_helper_boolean_composition_parts(
        "TRUE /\\ ~TemporalCommitPredicate",
        definitions,
        include_nested_negation=False,
    ) == ["negation TRUE /\\ ~TemporalCommitPredicate"]
    assert module.temporal_helper_boolean_composition_parts(
        "ConcreteTemporal /\\ ~TemporalCommitPredicate",
        definitions,
        include_nested_negation=False,
    ) == []


def test_cfg_correctness_envelope_shape_errors_allows_unary_temporal_concrete_formula(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (gst => <> committed)",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalControlFlowHelper == IF TRUE THEN TemporalCommitPredicate ELSE TemporalRecoveryPredicate",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalControlFlowHelper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "whole-body control-flow predicate-selection helper "
        "TemporalControlFlowHelper reaches "
        "TemporalControlFlowHelper through TemporalControlFlowHelper at "
        f"{tla}:8 is whole-body IF expression IF TRUE THEN "
        "TemporalCommitPredicate ELSE TemporalRecoveryPredicate; name concrete "
        "temporal predicates before composing allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalControlFlowHelper == [] (IF TRUE THEN TemporalCommitPredicate ELSE TemporalRecoveryPredicate)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalControlFlowHelper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "unary-temporal control-flow predicate-selection helper "
        "TemporalControlFlowHelper reaches TemporalControlFlowHelper through "
        f"TemporalControlFlowHelper at {tla}:8 is unary-temporal IF "
        "expression IF TRUE THEN TemporalCommitPredicate ELSE "
        "TemporalRecoveryPredicate; name concrete temporal predicates before "
        "composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalNestedControlFlow == ConcreteTemporal /\\ (IF ready THEN TemporalCommitPredicate ELSE TemporalRecoveryPredicate)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalNestedControlFlow",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "nested control-flow predicate-selection helper "
        "TemporalNestedControlFlow reaches TemporalNestedControlFlow through "
        f"TemporalNestedControlFlow at {tla}:9 contains nested IF expression "
        "IF ready THEN TemporalCommitPredicate ELSE TemporalRecoveryPredicate; "
        "name concrete temporal predicates before composing allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_enabled_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalNestedEnabledControlFlow == ConcreteTemporal /\\ (ENABLED TemporalCommitPredicate)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalNestedEnabledControlFlow",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "nested control-flow predicate-selection helper "
        "TemporalNestedEnabledControlFlow reaches "
        "TemporalNestedEnabledControlFlow through "
        f"TemporalNestedEnabledControlFlow at {tla}:8 contains nested ENABLED "
        "expression ENABLED TemporalCommitPredicate; name concrete temporal "
        "predicates before composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalLetAliasControlFlow == ConcreteTemporal /\\ "
                "(CASE ready -> LET selected == TemporalCommitPredicate IN selected "
                "[] OTHER -> LET fallback == TemporalRecoveryPredicate IN fallback)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalLetAliasControlFlow",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "nested control-flow predicate-selection helper "
        "TemporalLetAliasControlFlow reaches TemporalLetAliasControlFlow "
        f"through TemporalLetAliasControlFlow at {tla}:9 contains nested CASE "
        "expression CASE ready -> LET selected == TemporalCommitPredicate IN "
        "selected [] OTHER -> LET fallback == TemporalRecoveryPredicate IN "
        "fallback; name concrete temporal predicates before composing "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_result_wrapper_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalLetAliasResultWrapperControlFlow == ConcreteTemporal /\\ "
                "(CASE ready -> LET selected == TemporalCommitPredicate IN [] selected "
                "[] OTHER -> LET fallback == TemporalRecoveryPredicate IN <> fallback)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalLetAliasResultWrapperControlFlow",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "nested control-flow predicate-selection helper "
        "TemporalLetAliasResultWrapperControlFlow reaches "
        "TemporalLetAliasResultWrapperControlFlow through "
        f"TemporalLetAliasResultWrapperControlFlow at {tla}:9 contains nested "
        "CASE expression CASE ready -> LET selected == TemporalCommitPredicate "
        "IN [] selected [] OTHER -> LET fallback == TemporalRecoveryPredicate "
        "IN <> fallback; name concrete temporal predicates before composing "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_unary_temporal_control_flow_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalLetAliasUnaryControlFlow == ConcreteTemporal /\\ (LET selected == IF ready THEN TemporalCommitPredicate ELSE TemporalRecoveryPredicate IN [] selected)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalLetAliasUnaryControlFlow",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "unary-temporal control-flow predicate-selection helper "
        "TemporalLetAliasUnaryControlFlow reaches "
        "TemporalLetAliasUnaryControlFlow through "
        f"TemporalLetAliasUnaryControlFlow at {tla}:9 is unary-temporal IF "
        "expression IF ready THEN TemporalCommitPredicate ELSE "
        "TemporalRecoveryPredicate; name concrete temporal predicates before "
        "composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_let_alias_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalConcrete == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (LET selected == TemporalConcrete IN selected)",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:9 contains unary-temporal LET alias [] (LET selected == "
        "TemporalConcrete IN selected) aliases TemporalConcrete; name "
        "concrete temporal predicates before composing allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_unary_temporal_let_alias_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalConcrete == checked = ready",
                "OtherTemporal == ready = checked",
                "TemporalWrapper == [] (LET selected == TemporalConcrete IN selected)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope ==",
                "  /\\ TemporalWrapper",
                "  /\\ OtherTemporal",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "unary-temporal LET alias TemporalWrapper reaches TemporalWrapper "
        f"through TemporalWrapper at {tla}:8 contains [] (LET selected == "
        "TemporalConcrete IN selected) aliases TemporalConcrete; name "
        "concrete temporal predicates before composing allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_reference_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalHiddenLiteral == TRUE",
                "TemporalLetAliasReference == ConcreteTemporal /\\ "
                "(LET selected == TemporalHiddenLiteral IN selected)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalLetAliasReference",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalLetAliasReference reaches "
        "TemporalHiddenLiteral through TemporalLetAliasReference -> "
        f"TemporalHiddenLiteral at {tla}:7 is literal TRUE; keep literal, "
        "self-equality, self-inequality, and alias helpers out of allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_boolean_composition_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cases = (
        ("negation", "~TemporalCommitPredicate"),
        ("disjunction", "TemporalCommitPredicate \\/ TemporalRecoveryPredicate"),
        ("implication", "TemporalCommitPredicate => TemporalRecoveryPredicate"),
        ("equivalence", "TemporalCommitPredicate <=> TemporalRecoveryPredicate"),
    )
    for operator_name, temporal_body in cases:
        case_dir = tmp_path / operator_name
        case_dir.mkdir()
        tla = case_dir / "Sumeragi.tla"
        cfg = case_dir / "Sumeragi_tlc_fast.cfg"
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    "TemporalCommitPredicate == [] (Gst => <> Committed)",
                    "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                    f"TemporalBooleanWrapper == {temporal_body}",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalBooleanWrapper",
                    "EventuallyCommit == [] (Gst => <> Committed)",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
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
                    "INVARIANT TypeInvariant",
                    "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, cfg, "TLC"
        ) == [
            f"fast: TLC cfg {cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            "allowlisted temporal conjunct "
            f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
            f"{tla}:11 contains transitive temporal side-conjunct chain with "
            "whole-body temporal-helper boolean-composition helper "
            "TemporalBooleanWrapper reaches TemporalBooleanWrapper through "
            "TemporalBooleanWrapper at "
            f"{tla}:8 is whole-body {operator_name} {temporal_body}; name "
            "concrete temporal predicates before composing allowlisted "
            "temporal side-conjunct chains"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_gated_negated_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalBooleanWrapper == TRUE /\\ ~TemporalCommitPredicate",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalBooleanWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "whole-body temporal-helper boolean-composition helper "
        "TemporalBooleanWrapper reaches TemporalBooleanWrapper through "
        f"TemporalBooleanWrapper at {tla}:7 contains temporal-helper boolean "
        "composition negation TRUE /\\ ~TemporalCommitPredicate; name concrete "
        "temporal predicates before composing allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nested_temporal_helper_boolean_composition(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "TemporalCommitPredicate == [] (Gst => <> Committed)",
                "TemporalRecoveryPredicate == [] (Gst => <> Recovered)",
                "TemporalNestedWrapper == ConcreteTemporal /\\ (TemporalCommitPredicate \\/ TemporalRecoveryPredicate)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalNestedWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "whole-body temporal-helper boolean-composition helper "
        "TemporalNestedWrapper reaches TemporalNestedWrapper through "
        f"TemporalNestedWrapper at {tla}:9 contains temporal-helper boolean "
        "composition disjunction TemporalCommitPredicate \\/ "
        "TemporalRecoveryPredicate; name concrete temporal predicates before "
        "composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_concrete_boolean_formula_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcreteTemporalLeaf == committed => commitVotes >= CommitQuorum",
                "TemporalFormulaWrapper == [] ConcreteTemporalLeaf",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalFormulaWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_literal_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == TRUE",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "literal TRUE; temporal correctness-envelope exceptions must stay "
        "nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_static_if_literal_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == IF TRUE THEN TRUE ELSE FALSE",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "static IF literal TRUE; temporal correctness-envelope exceptions "
        "must stay nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_constant_relation_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == 1 \\in {1}",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "constant relation 1 \\in {1}; temporal correctness-envelope "
        "exceptions must stay nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_constant_relation_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == [] (1 \\in {1})",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "constant relation [] (1 \\in {1}); temporal correctness-envelope "
        "exceptions must stay nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_literal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == [] TRUE",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "literal TRUE; temporal correctness-envelope exceptions must stay "
        "nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_negated_unary_temporal_literal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == ~([] FALSE)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "literal TRUE; temporal correctness-envelope exceptions must stay "
        "nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_unary_temporal_literal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == [] ([] TRUE /\\ <> TRUE)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        f"allowlisted temporal conjunct EventuallyCommit at {tla}:9 is "
        "literal TRUE; temporal correctness-envelope exceptions must stay "
        "nontrivial"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_temporal_extra(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreStateSafetyEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateSafetyEnvelope",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "hidden coverage identifiers SumeragiConsensusCoreStateSafetyEnvelope "
        f"reaches TypeInvariant through SumeragiConsensusCoreStateSafetyEnvelope "
        f"at {tla}:7; keep TypeInvariant, generic correctness, and "
        "*Exactness identifiers out of allowlisted temporal side-conjunct "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreStateSafetyEnvelope(c) == SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (SumeragiConsensusCoreStateMatchesEnvelope /\\ SumeragiConsensusCoreStateSafetyEnvelope)",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:9 contains non-zero-arity temporal helper "
        "SumeragiConsensusCoreStateSafetyEnvelope reaches "
        f"SumeragiConsensusCoreStateSafetyEnvelope at {tla}:6 with arity 1; "
        "allowlisted temporal side-conjunct helper chains must use zero-arity "
        "predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_temporal_helper_call(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for temporal_body, expected_call in (
        (
            "[] (SumeragiConsensusCoreStateMatchesEnvelope => <> SumeragiConsensusCoreStateSafetyEnvelope(1))",
            "SumeragiConsensusCoreStateSafetyEnvelope(1)",
        ),
        (
            "[] (SumeragiConsensusCoreStateMatchesEnvelope <=> <> SumeragiConsensusCoreStateSafetyEnvelope(1))",
            "SumeragiConsensusCoreStateSafetyEnvelope(1)",
        ),
        (
            "[] (~SumeragiConsensusCoreStateSafetyEnvelope(1))",
            "SumeragiConsensusCoreStateSafetyEnvelope(1)",
        ),
    ):
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    "SumeragiConsensusCoreStateSafetyEnvelope(c) == SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == "
                    f"{temporal_body}",
                    "EventuallyCommit == [] (Gst => <> Committed)",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, cfg, "TLC"
        ) == [
            f"fast: TLC cfg {cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            "allowlisted temporal conjunct "
            f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
            f"{tla}:9 contains parameterized temporal helper call "
            "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
            f"reaches {expected_call} through "
            "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
            f"at {tla}:9; lift temporal helper calls behind zero-arity temporal "
            "predicates"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_parameterized_temporal_helper_call_with_expression_argument(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreStateSafetyEnvelope(c) == SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == "
                "[] SumeragiConsensusCoreStateSafetyEnvelope(ready = checked)",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:9 contains parameterized temporal helper call "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
        "reaches SumeragiConsensusCoreStateSafetyEnvelope(ready = checked) "
        "through SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
        f"at {tla}:9; lift temporal helper calls behind zero-arity temporal "
        "predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_parameterized_temporal_helper_call(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreStateSafetyEnvelope(c) == SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreTemporalCallHelper == SumeragiConsensusCoreStateSafetyEnvelope(1)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (SumeragiConsensusCoreStateMatchesEnvelope <=> SumeragiConsensusCoreTemporalCallHelper)",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains parameterized temporal helper call "
        "SumeragiConsensusCoreTemporalCallHelper reaches "
        "SumeragiConsensusCoreStateSafetyEnvelope(1) through "
        f"SumeragiConsensusCoreTemporalCallHelper at {tla}:7; lift temporal "
        "helper calls behind zero-arity temporal predicates"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_undefined_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] MissingTemporalPredicate",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:8 contains transitive temporal side-conjunct chain with "
        "undefined helper "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
        "reaches MissingTemporalPredicate through "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:8; define named concrete temporal predicates before composing "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_undefined_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalWrapper == [] MissingTemporalPredicate",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "Apalache"
    ) == [
        f"fast: Apalache cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:9 contains transitive temporal side-conjunct chain with "
        "undefined helper TemporalWrapper reaches MissingTemporalPredicate "
        f"through TemporalWrapper at {tla}:6; define named concrete temporal "
        "predicates before composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_quantified_undefined_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases, Other",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "QuantifiedTemporalLeaf == \\A c \\in Cases: MissingTemporalHelper",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] QuantifiedTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "undefined quantified helper QuantifiedTemporalLeaf reaches "
        "MissingTemporalHelper through QuantifiedTemporalLeaf at "
        f"{tla}:9; define named concrete temporal predicates before composing "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_existential_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == checked = c",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "ExistentialTemporalLeaf == \\E c \\in Cases: Predicate(c)",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] ExistentialTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "existential quantified helper ExistentialTemporalLeaf reaches "
        "\\E c \\in Cases: Predicate(c) through ExistentialTemporalLeaf at "
        f"{tla}:10; use universal quantified temporal predicates before "
        "composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_existential_quantified_temporal_operand(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == ready = committed",
                "Predicate(c) == checked = c",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (ConcreteTemporal /\\ \\E c \\in Cases: Predicate(c))",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "existential quantified helper "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
        "reaches \\E c \\in Cases: Predicate(c) through "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11; use universal quantified temporal predicates before "
        "composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_vacuous_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for case_name, formula, expected_line in (
        ("literal", "\\A c \\in Cases: TRUE", 9),
        ("unbounded_literal", "\\A c: TRUE", 9),
        ("unbounded_unary_temporal_literal", "\\E c: [] FALSE", 9),
        (
            "let_alias_literal",
            "\\A c \\in Cases: LET selected == TRUE IN selected",
            9,
        ),
        (
            "chained_let_alias_literal",
            "\\A c \\in Cases: LET first == TRUE second == first IN second",
            9,
        ),
        (
            "chained_let_alias_composed_literal",
            "\\A c \\in Cases: LET first == TRUE second == first /\\ TRUE IN second",
            9,
        ),
        ("self", "\\A c \\in Cases: checked = checked", 9),
        ("self_inequality_hash", "\\A c \\in Cases: checked # checked", 9),
        ("self_inequality_slash", "\\A c \\in Cases: checked /= checked", 9),
        ("self_membership_set", "\\A c \\in Cases: c \\in {c}", 9),
        ("self_membership_domain", "\\A c \\in Cases: c \\in Cases", 9),
        (
            "self_membership_comma_shared_domain",
            "\\A c, d \\in Cases: c \\in Cases",
            9,
        ),
        (
            "self_membership_unary_temporal",
            "\\A c \\in Cases: [] (c \\in {c})",
            9,
        ),
        (
            "self_nonmembership_negated",
            "\\A c \\in Cases: ~(c \\notin {c})",
            9,
        ),
        ("empty_set_nonmembership", "\\A c \\in Cases: c \\notin {}", 9),
        (
            "empty_set_nonmembership_unary_temporal",
            "\\A c \\in Cases: [] (c \\notin {})",
            9,
        ),
        (
            "empty_set_membership_negated",
            "\\A c \\in Cases: ~(c \\in {})",
            9,
        ),
        (
            "self_membership_boolean_composed",
            "\\A c \\in Cases: c \\in Cases /\\ c \\notin {}",
            9,
        ),
        (
            "self_membership_identity_gated",
            "\\A c \\in Cases: TRUE /\\ c \\in Cases",
            9,
        ),
        (
            "self_membership_truth_implication",
            "\\A c \\in Cases: c \\notin Cases => FALSE",
            9,
        ),
        ("empty_domain", "\\A c \\in {}: c = committed", 9),
        (
            "empty_domain_unary_temporal",
            "\\A c \\in {}: [] (c = committed)",
            9,
        ),
        ("singleton_domain_equality", "\\A c \\in {1}: c = 1", 9),
        (
            "singleton_tuple_domain_equality",
            "\\A t \\in {<<1, 2>>}: t = <<1, 2>>",
            9,
        ),
        (
            "singleton_domain_comma_shared_domain",
            "\\A c, d \\in {1}: c = 1",
            9,
        ),
        ("singleton_domain_reversed_equality", "\\A c \\in {1}: 1 = c", 9),
        (
            "singleton_domain_unary_temporal",
            "\\A c \\in {1}: [] (c = 1)",
            9,
        ),
        (
            "singleton_domain_negated_inequality",
            "\\A c \\in {1}: ~(c # 1)",
            9,
        ),
        (
            "singleton_domain_boolean_composed",
            "\\A c \\in {1}: c = 1 /\\ 1 = c",
            9,
        ),
        (
            "singleton_domain_identity_gated",
            "\\A c \\in {1}: TRUE => c = 1",
            9,
        ),
        (
            "singleton_domain_truth_equivalence",
            "\\A c \\in {1}: c # 1 <=> FALSE",
            9,
        ),
    ):
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "Sumeragi.tla"
        local_cfg = case_dir / "Sumeragi_tlc_fast.cfg"
        local_cfg.write_text(cfg.read_text(encoding="utf-8"), encoding="utf-8")
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "CONSTANT Cases",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    f"VacuousTemporalLeaf == {formula}",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] VacuousTemporalLeaf",
                    "EventuallyCommit == [] (Gst => <> Committed)",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, local_cfg, "TLC"
        ) == [
            f"fast: TLC cfg {local_cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            "allowlisted temporal conjunct "
            "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
            f"at {tla}:10 contains transitive temporal side-conjunct chain "
            "with vacuous quantified helper VacuousTemporalLeaf reaches "
            f"{formula} through VacuousTemporalLeaf at {tla}:{expected_line}; "
            "keep literal and self-equality, self-inequality, "
            "empty-domain, singleton-domain, self-membership, or empty-set membership quantified helper "
            "bodies out of allowlisted temporal side-conjunct chains"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_duplicate_bound_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporalPredicate(c) == c = committed",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "DuplicateBoundTemporalLeaf == \\A c, c \\in Cases: ConcreteTemporalPredicate(c)",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] DuplicateBoundTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "duplicate quantified helper binding DuplicateBoundTemporalLeaf reaches "
        "\\A c, c \\in Cases: ConcreteTemporalPredicate(c) duplicates bound c "
        f"through DuplicateBoundTemporalLeaf at {tla}:10; bind each quantified "
        "identifier once before composing allowlisted temporal side-conjunct "
        "chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_static_wrapped_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == c = committed",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "StaticWrappedTemporalLeaf == SUBSET (\\A c \\in Cases: Predicate(c))",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] StaticWrappedTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "static-wrapper quantified formula StaticWrappedTemporalLeaf reaches "
        "SUBSET wraps \\A c \\in Cases: Predicate(c) through "
        f"StaticWrappedTemporalLeaf at {tla}:10; name quantified temporal "
        "predicates before composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_structured_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == c = committed",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "StructuredTemporalLeaf == [Field |-> \\A c \\in Cases: Predicate(c)]",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] StructuredTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "structured quantified formula StructuredTemporalLeaf reaches record "
        "literal contains \\A c \\in Cases: Predicate(c) through "
        f"StructuredTemporalLeaf at {tla}:10; name quantified temporal "
        "predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_structured_control_flow_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporal == [] (Gst => <> Done)",
                "FallbackTemporal == [] (Gst => <> Fallback)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "StructuredTemporalLeaf == [Field |-> IF Ready THEN ConcreteTemporal ELSE FallbackTemporal]",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] StructuredTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "structured control-flow predicate-selection helper "
        "StructuredTemporalLeaf reaches record literal contains IF expression "
        "IF Ready THEN ConcreteTemporal ELSE FallbackTemporal through "
        f"StructuredTemporalLeaf at {tla}:10; name concrete temporal "
        "predicates before placing them in structured helper operands"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unused_bound_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases, Other",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "ConcreteTemporalPredicate(c) == c = committed",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "UnusedBoundTemporalLeaf == \\A c \\in Cases, d \\in Other: ConcreteTemporalPredicate(c)",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] UnusedBoundTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "unused quantified helper binding UnusedBoundTemporalLeaf reaches "
        "\\A c \\in Cases, d \\in Other: ConcreteTemporalPredicate(c) omits "
        f"bound d through UnusedBoundTemporalLeaf at {tla}:10; use every bound identifier "
        "inside quantified temporal predicates before composing allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_control_flow_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == ready = c",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "ControlFlowTemporalLeaf == \\A c \\in Cases: IF c = c THEN Predicate(c) ELSE Predicate(c)",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] ControlFlowTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "control-flow quantified helper ControlFlowTemporalLeaf reaches "
        "\\A c \\in Cases: IF c = c THEN Predicate(c) ELSE Predicate(c) uses "
        f"IF through ControlFlowTemporalLeaf at {tla}:10; name concrete "
        "quantified temporal predicates instead of selecting predicates "
        "inside quantified helper bodies"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_nontransparent_let_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "NonTransparentLetTemporalLeaf == \\A c \\in Cases: LET local(x) == TRUE IN local(c)",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] NonTransparentLetTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "control-flow quantified helper NonTransparentLetTemporalLeaf reaches "
        "\\A c \\in Cases: LET local(x) == TRUE IN local(c) uses LET through "
        f"NonTransparentLetTemporalLeaf at {tla}:9; name concrete quantified "
        "temporal predicates instead of selecting predicates inside "
        "quantified helper bodies"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_negated_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == checked = c",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "NegatedQuantifiedTemporalLeaf == ~(\\A c \\in Cases: Predicate(c))",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] NegatedQuantifiedTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "negated quantified helper NegatedQuantifiedTemporalLeaf reaches "
        "\\A c \\in Cases: Predicate(c) under 1 top-level negation(s) through "
        f"NegatedQuantifiedTemporalLeaf at {tla}:10; compose positive "
        "quantified temporal predicates before allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_negated_quantified_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == checked = c",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "LetAliasNegatedQuantifiedTemporalLeaf == LET selected == \\A c \\in Cases: Predicate(c) IN ~selected",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] LetAliasNegatedQuantifiedTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "negated quantified helper LetAliasNegatedQuantifiedTemporalLeaf "
        "reaches \\A c \\in Cases: Predicate(c) under 1 top-level "
        "negation(s) through LetAliasNegatedQuantifiedTemporalLeaf at "
        f"{tla}:10; compose positive quantified temporal predicates before "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_negated_quantified_operand(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "CONSTANT Cases",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "Predicate(c) == checked = c",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] (~(\\A c \\in Cases: Predicate(c)))",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "negated quantified helper "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
        "reaches \\A c \\in Cases: Predicate(c) under 1 top-level "
        "negation(s) through "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10; compose positive quantified temporal predicates before "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_undefined_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalKnown == [] (gst => <> committed)",
                "TemporalWrapper == [] (TemporalKnown \\/ MissingTemporalPredicate)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "undefined helper TemporalWrapper reaches MissingTemporalPredicate "
        f"through TemporalWrapper at {tla}:7; define named concrete temporal "
        "predicates before composing allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_uppercase_temporal_formula_atoms(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreStateSafetyEnvelope == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_duplicate_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "TemporalDuplicateWrapper ==",
                "  /\\ TemporalLeaf",
                "  /\\ TemporalLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalDuplicateWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:12 contains transitive temporal side-conjunct chain with "
        "repeated helper conjunct TemporalDuplicateWrapper reaches "
        "TemporalDuplicateWrapper through TemporalDuplicateWrapper at "
        f"{tla}:8 repeats TemporalLeaf; remove duplicate helper conjuncts so "
        "every temporal obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_wrapped_duplicate_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "TemporalDuplicateWrapper == [] (TemporalLeaf /\\ [] TemporalLeaf)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalDuplicateWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "repeated helper conjunct TemporalDuplicateWrapper reaches "
        "TemporalDuplicateWrapper through TemporalDuplicateWrapper at "
        f"{tla}:7 repeats TemporalLeaf; remove duplicate helper conjuncts so "
        "every temporal obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_duplicate_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "TemporalDuplicateWrapper == TemporalLeaf /\\ (LET selected == TemporalLeaf IN selected)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalDuplicateWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "repeated helper conjunct TemporalDuplicateWrapper reaches "
        "TemporalDuplicateWrapper through TemporalDuplicateWrapper at "
        f"{tla}:7 repeats TemporalLeaf; remove duplicate helper conjuncts so "
        "every temporal obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_compound_duplicate_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "OtherTemporal == [] (committed => gst)",
                "TemporalDuplicateWrapper == [] ((TemporalLeaf /\\ TemporalLeaf) \\/ OtherTemporal)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalDuplicateWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "repeated helper conjunct TemporalDuplicateWrapper reaches "
        "TemporalDuplicateWrapper through TemporalDuplicateWrapper at "
        f"{tla}:8 repeats TemporalLeaf; remove duplicate helper conjuncts so "
        "every temporal obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_repeated_boolean_operand_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "TemporalDuplicateWrapper == TemporalLeaf \\/ [] TemporalLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalDuplicateWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "repeated helper operand TemporalDuplicateWrapper reaches "
        "TemporalDuplicateWrapper through TemporalDuplicateWrapper at "
        f"{tla}:7 repeats TemporalLeaf; remove duplicate helper operands so "
        "every temporal obligation is counted once"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_contradictory_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "TemporalContradictoryWrapper == TemporalLeaf /\\ ~TemporalLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalContradictoryWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "contradictory helper operand TemporalContradictoryWrapper reaches "
        "TemporalContradictoryWrapper through TemporalContradictoryWrapper "
        f"at {tla}:7 pairs TemporalLeaf with ~TemporalLeaf; name concrete "
        "non-contradictory temporal predicates before composing allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_excluded_middle_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == [] (gst => <> committed)",
                "TemporalExcludedMiddleWrapper == TemporalLeaf \\/ ~TemporalLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalExcludedMiddleWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "excluded-middle helper operand TemporalExcludedMiddleWrapper reaches "
        "TemporalExcludedMiddleWrapper through TemporalExcludedMiddleWrapper "
        f"at {tla}:7 pairs TemporalLeaf with ~TemporalLeaf; name concrete "
        "non-tautological temporal predicates before composing allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_complementary_equivalence_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLeaf == checked /\\ ready",
                "TemporalComplementaryEquivalenceWrapper == TemporalLeaf <=> ~TemporalLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalComplementaryEquivalenceWrapper",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "complementary-equivalence helper operand "
        "TemporalComplementaryEquivalenceWrapper reaches "
        "TemporalComplementaryEquivalenceWrapper through "
        f"TemporalComplementaryEquivalenceWrapper at {tla}:7 pairs "
        "TemporalLeaf with ~TemporalLeaf under equivalence; name concrete "
        "non-vacuous temporal predicates before composing allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_repeated_state_terms_in_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalConcrete ==",
                "  /\\ gst",
                "  /\\ gst",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalConcrete",
                "EventuallyCommit == [] (gst => <> committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "Apalache"
    ) == []


def test_cfg_correctness_envelope_shape_errors_rejects_structured_literal_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "LiteralLeaf == TRUE",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "StructuredTemporalLeaf == [Field |-> LiteralLeaf]",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] StructuredTemporalLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper StructuredTemporalLeaf reaches LiteralLeaf through "
        f"StructuredTemporalLeaf -> LiteralLeaf at {tla}:6 is literal TRUE; "
        "keep literal, self-equality, self-inequality, and alias helpers out "
        "of allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_vacuous_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalConcrete == checked = ready",
                "TemporalLiteralLeaf == [] (TRUE /\\ TRUE)",
                "TemporalAliasLeaf == TemporalConcrete",
                "TemporalWrapper ==",
                "  /\\ TemporalLiteralLeaf",
                "  /\\ TemporalAliasLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:14 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalAliasLeaf through "
        f"TemporalWrapper -> TemporalAliasLeaf at {tla}:8 aliases "
        "TemporalConcrete, TemporalWrapper reaches TemporalLiteralLeaf through "
        f"TemporalWrapper -> TemporalLiteralLeaf at {tla}:7 is literal TRUE; "
        "keep literal, self-equality, self-inequality, and alias helpers out "
        "of allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_static_if_literal_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalStaticIfLeaf == IF FALSE THEN TRUE ELSE FALSE",
                "TemporalWrapper == [] TemporalStaticIfLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalStaticIfLeaf through "
        f"TemporalWrapper -> TemporalStaticIfLeaf at {tla}:6 is static IF "
        "literal FALSE; keep literal, self-equality, self-inequality, and "
        "alias helpers out of allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_constant_relation_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalConstantRelationLeaf == TRUE = TRUE",
                "TemporalWrapper == [] TemporalConstantRelationLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalConstantRelationLeaf "
        "through TemporalWrapper -> TemporalConstantRelationLeaf at "
        f"{tla}:6 is constant relation TRUE = TRUE; keep literal, "
        "self-equality, self-inequality, and alias helpers out of "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_transitive_unary_temporal_constant_relation_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalConstantRelationLeaf == [] (TRUE = TRUE)",
                "TemporalWrapper == [] TemporalConstantRelationLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalConstantRelationLeaf "
        "through TemporalWrapper -> TemporalConstantRelationLeaf at "
        f"{tla}:6 is constant relation [] (TRUE = TRUE); keep literal, "
        "self-equality, self-inequality, and alias helpers out of "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_negated_transitive_vacuous_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLiteralLeaf == [] (TRUE /\\ TRUE)",
                "TemporalNegatedWrapper == [] (~TemporalLiteralLeaf)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalNegatedWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalNegatedWrapper reaches TemporalLiteralLeaf "
        f"through TemporalNegatedWrapper -> TemporalLiteralLeaf at {tla}:6 "
        "is literal TRUE; keep literal, self-equality, self-inequality, and "
        "alias helpers out of allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_single_conjunct_temporal_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalFormulaLeaf == checked = ready",
                "TemporalWrapper ==",
                "  /\\ TemporalFormulaLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:11 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalWrapper through "
        f"TemporalWrapper at {tla}:8 aliases TemporalFormulaLeaf through a "
        "single helper conjunct; keep literal, self-equality, "
        "self-inequality, and alias helpers out of allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_unary_temporal_single_conjunct_temporal_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalFormulaLeaf == checked = ready",
                "TemporalWrapper == [] (/\\ TemporalFormulaLeaf)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalWrapper through "
        f"TemporalWrapper at {tla}:7 aliases TemporalFormulaLeaf through a "
        "single helper conjunct; keep literal, self-equality, "
        "self-inequality, and alias helpers out of allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_gated_single_conjunct_temporal_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalFormulaLeaf == checked = ready",
                "TemporalWrapper == TRUE => (/\\ TemporalFormulaLeaf)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalWrapper through "
        f"TemporalWrapper at {tla}:7 aliases TemporalFormulaLeaf through a "
        "single helper conjunct; keep literal, self-equality, "
        "self-inequality, and alias helpers out of allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_literal_gated_zero_arity_temporal_alias_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalFormulaLeaf == checked = ready",
                "TemporalWrapper == TRUE /\\ TemporalFormulaLeaf",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalWrapper",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:10 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalWrapper reaches TemporalWrapper through "
        f"TemporalWrapper at {tla}:7 aliases TemporalFormulaLeaf through a "
        "literal-gated helper operand; keep literal, self-equality, "
        "self-inequality, and alias helpers out of allowlisted temporal "
        "side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_allows_direct_formula_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalFormulaLeaf == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalFormulaLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors("fast", tla, cfg, "TLC")
        == []
    )


def test_cfg_correctness_envelope_shape_errors_rejects_let_alias_temporal_literal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalLetLiteral == [] (LET selected == TRUE IN selected)",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalLetLiteral",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        f"SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:9 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalLetLiteral reaches TemporalLetLiteral through "
        f"TemporalLetLiteral at {tla}:6 is literal TRUE; keep literal, "
        "self-equality, self-inequality, and alias helpers out of allowlisted "
        "temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_self_equality_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "TemporalSelfEqualityLeaf == checked = checked",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalSelfEqualityLeaf",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
        "allowlisted temporal conjunct "
        "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope at "
        f"{tla}:9 contains transitive temporal side-conjunct chain with "
        "vacuous helper TemporalSelfEqualityLeaf reaches "
        "TemporalSelfEqualityLeaf through TemporalSelfEqualityLeaf at "
        f"{tla}:6 is self-equality checked = checked; keep literal, "
        "self-equality, self-inequality, and alias helpers out of "
        "allowlisted temporal side-conjunct chains"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_self_inequality_temporal_helper(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    cfg.write_text(
        "\n".join(
            [
                "INIT Init",
                "NEXT Next",
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    for case_name, formula in (
        ("hash", "checked # checked"),
        ("slash", "checked /= checked"),
    ):
        case_dir = tmp_path / case_name
        case_dir.mkdir()
        tla = case_dir / "Sumeragi.tla"
        local_cfg = case_dir / "Sumeragi_tlc_fast.cfg"
        local_cfg.write_text(cfg.read_text(encoding="utf-8"), encoding="utf-8")
        tla.write_text(
            "\n".join(
                [
                    "---- MODULE Sumeragi ----",
                    "Init == TRUE",
                    "Next == TRUE",
                    "TypeInvariant == TRUE",
                    "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                    f"TemporalSelfInequalityLeaf == {formula}",
                    "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                    "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                    "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] TemporalSelfInequalityLeaf",
                    "EventuallyCommit == [] (Gst => <> Committed)",
                    "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                    "  /\\ TypeInvariant",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                    "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                    "  /\\ EventuallyCommit",
                    "====",
                ]
            ),
            encoding="utf-8",
        )

        assert module.cfg_correctness_envelope_shape_errors(
            "fast", tla, local_cfg, "TLC"
        ) == [
            f"fast: TLC cfg {local_cfg}:4 references correctness envelope "
            "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but "
            "allowlisted temporal conjunct "
            "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope "
            f"at {tla}:9 contains transitive temporal side-conjunct chain "
            "with vacuous helper TemporalSelfInequalityLeaf reaches "
            "TemporalSelfInequalityLeaf through TemporalSelfInequalityLeaf at "
            f"{tla}:6 is self-inequality {formula}; keep literal, "
            "self-equality, self-inequality, and alias helpers out of "
            "allowlisted temporal side-conjunct chains"
        ]


def test_cfg_correctness_envelope_shape_errors_rejects_stale_temporal_allowlist(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateMatchesEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  /\\ SumeragiConsensusCoreStateMatchesEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateMatchesEnvelope",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but its "
        "temporal allowlist is stale for EventuallyCommit"
    ]


def test_cfg_correctness_envelope_shape_errors_rejects_top_level_direct_alias_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_tlc_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateSafetyEnvelope == checked = ready",
                "SumeragiConsensusCoreAlwaysMatchesExactness ==",
                "  SumeragiConsensusCoreStateSafetyEnvelope",
                "SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope == [] SumeragiConsensusCoreStateSafetyEnvelope",
                "EventuallyCommit == [] (Gst => <> Committed)",
                "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesExactness",
                "  /\\ SumeragiConsensusCoreAlwaysMatchesStateAndTemporalSafetyEnvelope",
                "  /\\ EventuallyCommit",
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
                "INVARIANT TypeInvariant",
                "PROPERTY SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_correctness_envelope_shape_errors(
        "fast", tla, cfg, "TLC"
    ) == [
        f"fast: TLC cfg {cfg}:4 references correctness envelope "
        "SumeragiConsensusCoreAlwaysMatchesCorrectnessEnvelope, but exactness "
        f"conjunct SumeragiConsensusCoreAlwaysMatchesExactness at {tla}:7 "
        "aliases SumeragiConsensusCoreStateSafetyEnvelope; inline concrete "
        "model predicates directly"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_mixed_generic_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectMixedSafety.tla"
    cfg = tmp_path / "SumeragiDirectMixedSafety_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectMixedSafety ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "Safety == ModelPredicate",
                "DirectMixedExactness ==",
                "  /\\ Safety",
                "  /\\ ModelPredicate",
                "DirectMixedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ModelPredicate",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectMixedExactness",
                "INVARIANT DirectMixedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "direct-mixed-safety-fast", tla, cfg, "Apalache"
    ) == [
        f"direct-mixed-safety-fast: Apalache cfg {cfg}:4 references direct "
        f"exactness check DirectMixedExactness at {tla}:8 mentions generic "
        "Safety; compose concrete model predicates directly"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_direct_alias(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectAlias.tla"
    cfg = tmp_path / "SumeragiDirectAlias_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectAlias ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "DirectAliasExactness == ModelPredicate",
                "DirectAliasCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectAliasExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectAliasExactness",
                "INVARIANT DirectAliasCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "direct-alias-fast", tla, cfg, "TLC"
    ) == [
        f"direct-alias-fast: TLC cfg {cfg}:4 references direct exactness "
        f"check DirectAliasExactness at {tla}:6 aliases ModelPredicate; "
        "inline concrete model predicates directly"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_nested_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectNestedExactness.tla"
    cfg = tmp_path / "SumeragiDirectNestedExactness_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectNestedExactness ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == checked = ready",
                "ChildExactness ==",
                "  /\\ ConcretePredicate",
                "ParentExactness ==",
                "  /\\ ChildExactness",
                "ParentCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParentExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParentExactness",
                "INVARIANT ParentCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "direct-nested-exactness-fast", tla, cfg, "Apalache"
    ) == [
        f"direct-nested-exactness-fast: Apalache cfg {cfg}:4 references "
        f"direct exactness check ParentExactness at {tla}:9 composes "
        "nested exactness ChildExactness; inline concrete model predicates "
        "directly"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_duplicate_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectDuplicate.tla"
    cfg = tmp_path / "SumeragiDirectDuplicate_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectDuplicate ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "FirstPredicate == TRUE",
                "SecondPredicate == TRUE",
                "DirectDuplicateExactness ==",
                "  /\\ FirstPredicate",
                "  /\\ SecondPredicate",
                "  /\\ FirstPredicate",
                "DirectDuplicateCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectDuplicateExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectDuplicateExactness",
                "INVARIANT DirectDuplicateCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "direct-duplicate-fast", tla, cfg, "TLC"
    ) == [
        f"direct-duplicate-fast: TLC cfg {cfg}:4 references direct exactness "
        f"check DirectDuplicateExactness at {tla}:8 repeats exactness conjunct "
        "FirstPredicate; remove duplicate conjuncts so every obligation is "
        "counted once"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_aliased_exactness_conjunct(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectAliasedConjunct.tla"
    cfg = tmp_path / "SumeragiDirectAliasedConjunct_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectAliasedConjunct ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ConcretePredicate == TRUE",
                "WrappedPredicate == ConcretePredicate",
                "WrappedExactness ==",
                "  /\\ WrappedPredicate",
                "WrappedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ WrappedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT WrappedExactness",
                "INVARIANT WrappedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "direct-aliased-conjunct-fast", tla, cfg, "TLC"
    ) == [
        f"direct-aliased-conjunct-fast: TLC cfg {cfg}:4 references direct "
        f"exactness check WrappedExactness at {tla}:8 contains aliased "
        f"exactness conjunct WrappedPredicate at {tla}:6 aliases "
        "ConcretePredicate; inline concrete model predicates directly"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_type_invariant_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiDirectTypeExactness.tla"
    cfg = tmp_path / "SumeragiDirectTypeExactness_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiDirectTypeExactness ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "ModelPredicate == checked = ready",
                "DirectTypeExactness ==",
                "  /\\ TypeInvariant",
                "  /\\ ModelPredicate",
                "DirectTypeCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ DirectTypeExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT DirectTypeExactness",
                "INVARIANT DirectTypeCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "direct-type-exactness-fast", tla, cfg, "TLC"
    ) == [
        f"direct-type-exactness-fast: TLC cfg {cfg}:4 references direct "
        f"exactness check DirectTypeExactness at {tla}:7 mentions "
        "TypeInvariant; keep type invariants in *CorrectnessEnvelope operators"
    ]


def test_cfg_direct_exactness_shape_errors_rejects_unnamed_parameterized_checks(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiParameterizedExactness.tla"
    cfg = tmp_path / "SumeragiParameterizedExactness_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiParameterizedExactness ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "Cases == {1, 2}",
                "ActualResult(c) == TRUE",
                "SpecResult(c) == TRUE",
                "ParameterizedExactness ==",
                "  /\\ TRUE",
                "  /\\ FALSE",
                "ParameterizedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ ParameterizedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT ParameterizedExactness",
                "INVARIANT ParameterizedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_shape_errors(
        "parameterized-exactness-fast", tla, cfg, "Apalache"
    ) == [
        f"parameterized-exactness-fast: Apalache cfg {cfg}:4 references "
        f"direct exactness check ParameterizedExactness at {tla}:9 is literal "
        "FALSE"
    ]


def test_cfg_direct_exactness_envelope_pairing_errors_rejects_unpaired_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiUnpairedExactness.tla"
    cfg = tmp_path / "SumeragiUnpairedExactness_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiUnpairedExactness ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "UnpairedExactness ==",
                "  /\\ ModelPredicate",
                "PairedExactness ==",
                "  /\\ ModelPredicate",
                "ModelPredicate == checked = ready",
                "PairedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ PairedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnpairedExactness",
                "INVARIANT PairedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_envelope_pairing_errors(
        "unpaired-exactness-fast", tla, cfg, "Apalache"
    ) == [
        f"unpaired-exactness-fast: Apalache cfg {cfg}:4 references direct "
        "exactness check UnpairedExactness, but no checked correctness envelope "
        "in that CFG composes it"
    ]


def test_cfg_direct_exactness_envelope_pairing_errors_ignores_string_literal_exactness_names(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiStringExactnessName.tla"
    cfg = tmp_path / "SumeragiStringExactnessName_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiStringExactnessName ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "UnpairedExactness ==",
                "  /\\ ModelPredicate",
                "ModelPredicate == checked = ready",
                'StringMentionCorrectnessEnvelope == TypeInvariant /\\ "UnpairedExactness"',
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
                "INVARIANT TypeInvariant",
                "INVARIANT UnpairedExactness",
                "INVARIANT StringMentionCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert module.cfg_direct_exactness_envelope_pairing_errors(
        "string-exactness-name-fast", tla, cfg, "Apalache"
    ) == [
        f"string-exactness-name-fast: Apalache cfg {cfg}:4 references direct "
        "exactness check UnpairedExactness, but no checked correctness envelope "
        "in that CFG composes it"
    ]


def test_cfg_direct_exactness_envelope_pairing_errors_accepts_paired_exactness(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "SumeragiPairedExactness.tla"
    cfg = tmp_path / "SumeragiPairedExactness_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE SumeragiPairedExactness ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "PairedExactness ==",
                "  /\\ ModelPredicate",
                "ModelPredicate == checked = ready",
                "PairedCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ PairedExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT PairedExactness",
                "INVARIANT PairedCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_direct_exactness_envelope_pairing_errors(
            "paired-exactness-fast", tla, cfg, "TLC"
        )
        == []
    )


def test_cfg_correctness_envelope_shape_errors_accepts_fast_exactness_envelope(
    tmp_path: Path,
) -> None:
    module = load_coverage_module()
    tla = tmp_path / "Sumeragi.tla"
    cfg = tmp_path / "Sumeragi_fast.cfg"
    tla.write_text(
        "\n".join(
            [
                "---- MODULE Sumeragi ----",
                "Init == TRUE",
                "Next == TRUE",
                "TypeInvariant == TRUE",
                "SumeragiConsensusCoreStateSafetyEnvelope == checked = ready",
                "SumeragiConsensusCoreExactness ==",
                "  /\\ SumeragiConsensusCoreStateSafetyEnvelope",
                "SumeragiConsensusCoreFastCorrectnessEnvelope ==",
                "  /\\ TypeInvariant",
                "  /\\ SumeragiConsensusCoreExactness",
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
                "INVARIANT TypeInvariant",
                "INVARIANT SumeragiConsensusCoreFastCorrectnessEnvelope",
            ]
        ),
        encoding="utf-8",
    )

    assert (
        module.cfg_correctness_envelope_shape_errors(
            "fast", tla, cfg, "Apalache"
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


def test_pr_tlc_cross_check_errors_rejects_missing_non_allowlisted_mode() -> None:
    module = load_coverage_module()

    errors = module.pr_tlc_cross_check_errors(
        {"fast", "deep", "fork-npos"},
        {"fast"},
        {"fast"},
    )

    assert len(errors) == 2
    assert errors[0] == (
        "Sumeragi PR baseline modes without TLC runner cases "
        "(not explicitly Apalache-only):\n"
        "  - fork-npos"
    )
    assert errors[1] == (
        "Sumeragi PR baseline modes without README TLC commands "
        "(not explicitly Apalache-only):\n"
        "  - fork-npos"
    )


def test_pr_tlc_cross_check_errors_accepts_documented_apalache_only_deep() -> None:
    module = load_coverage_module()

    assert (
        module.pr_tlc_cross_check_errors(
            {"fast", "deep", "fork-npos"},
            {"fast", "fork-npos"},
            {"fast", "fork-npos"},
        )
        == []
    )


def test_pr_tlc_cross_check_errors_rejects_stale_or_routed_allowlist() -> None:
    module = load_coverage_module()

    errors = module.pr_tlc_cross_check_errors(
        {"fast"},
        {"fast", "deep"},
        {"fast", "deep"},
    )

    assert errors == [
        "Sumeragi Apalache-only PR mode allowlist entries are stale:\n"
        "  - deep",
        "Sumeragi Apalache-only PR modes unexpectedly have TLC runner cases:\n"
        "  - deep",
        "Sumeragi Apalache-only PR modes unexpectedly have README TLC commands:\n"
        "  - deep",
    ]


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
