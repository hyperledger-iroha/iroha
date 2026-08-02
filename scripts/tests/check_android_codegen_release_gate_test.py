"""Static contracts for deterministic Android binding generation in release CI."""

from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def read(relative: str) -> str:
    """Read one repository contract as UTF-8."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def test_android_codegen_gate_uses_two_clean_isolated_replays() -> None:
    """Binding generation must compare two detached HEAD worktrees byte for byte."""

    gate = read("ci/check_android_codegen.sh")
    assert "deterministic binding generation requires a clean checkout" in gate
    assert 'HEAD_COMMIT="$(git rev-parse --verify \'HEAD^{commit}\')"' in gate
    assert gate.count('create_replay_worktree "${') == 2
    assert gate.count('run_replay "${') == 2
    assert 'diff -ru "${FIRST_WORKTREE}/${DOCS_REL}"' in gate
    assert 'cmp -s "${FIRST_WORKTREE}/${SUMMARY_REL}"' in gate
    assert "two clean Android binding generations produced different bytes" in gate
    assert "two clean Android parity summaries disagreed" in gate


def test_android_codegen_gate_fails_on_missing_or_extra_outputs() -> None:
    """Every tracked output and the mandatory parity/hash artifacts fail closed."""

    gate = read("ci/check_android_codegen.sh")
    assert "status --porcelain=v1 --untracked-files=all" in gate
    assert "generator mutated an unexpected path" in gate
    assert '"${worktree}/${DOCS_REL}/codegen_hash_tree.json"' in gate
    assert '"${worktree}/${DOCS_REL}/codegen_manifest_metadata.json"' in gate
    assert '"${worktree}/${SUMMARY_REL}"' in gate
    assert 'if [[ ! -f "${required}" || -L "${required}" ]]' in gate
    assert 'if [[ -f "${HASH_TREE_SOURCE}" ]]' not in gate


def test_android_codegen_archive_has_canonical_reproducible_metadata() -> None:
    """The staged archive must not inherit timestamps, owners, modes, or links."""

    gate = read("ci/check_android_codegen.sh")
    for marker in (
        'gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0)',
        "format=tarfile.PAX_FORMAT",
        "info.uid = 0",
        "info.gid = 0",
        'info.uname = ""',
        'info.gname = ""',
        "info.mtime = 0",
        "Android generated-doc archive must not contain symlinks",
    ):
        assert marker in gate
    assert gate.count('build_deterministic_archive "${') == 2
    assert 'cmp -s "${FIRST_ARCHIVE}" "${SECOND_ARCHIVE}"' in gate
    assert "two clean Android documentation archives produced different bytes" in gate
    assert "tar -czf" not in gate


def test_openapi_workflow_executes_binding_replay_after_openapi_replay() -> None:
    """The canonical release-input workflow must actually execute both gates."""

    workflow = read(".github/workflows/openapi.yml")
    openapi = workflow.index("run: bash ci/check_openapi_spec.sh")
    bindings = workflow.index("run: bash ci/check_android_codegen.sh")
    assert openapi < bindings
    assert "timeout-minutes: 120" in workflow
