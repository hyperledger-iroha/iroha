"""Contracts for full-tree deterministic SoraFS fixture regeneration."""

from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def fixture_gate() -> str:
    """Return the fixture release gate source."""

    return (REPO_ROOT / "ci/check_sorafs_fixtures.sh").read_text(encoding="utf-8")


def test_fixed_path_generators_run_in_two_detached_clean_worktrees() -> None:
    """Generators without output flags must never mutate or reuse the caller tree."""

    gate = fixture_gate()
    assert "deterministic fixture generation requires a clean checkout" in gate
    assert 'fixture_commit="$(git rev-parse --verify \'HEAD^{commit}\')"' in gate
    assert 'git -C "${repo_root}" worktree add --quiet --detach' in gate
    assert "for fixed_fixture_pass in 1 2; do" in gate
    assert 'create_fixture_worktree "${pass_worktree}"' in gate
    assert 'run_fixed_path_fixture_generators "${pass_worktree}"' in gate
    fixed_runner = gate.split("run_fixed_path_fixture_generators()", 1)[1].split(
        "snapshot_fixed_path_fixture_roots()", 1
    )[0]
    for generator in (
        "--bin export_vectors",
        "--bin provider_admission_fixtures",
        "--example gen_pin_snapshot",
    ):
        assert fixed_runner.count(generator) == 1
    assert "cargo run --locked" in fixed_runner
    assert "fixed-path generators changed checked-in bytes or emitted an unexpected path" in fixed_runner
    assert 'status --porcelain=v1 --untracked-files=all' in fixed_runner


def test_fixed_path_fixture_inventory_is_complete_and_byte_compared() -> None:
    """Every fixed generator output root must match HEAD and the independent replay."""

    gate = fixture_gate()
    for relative in (
        "fixtures/sorafs_chunker",
        "fuzz/sorafs_chunker",
        "fixtures/sorafs_manifest/provider_admission",
        "crates/iroha_core/tests/fixtures/sorafs_pin_registry",
    ):
        assert f'"{relative}"' in gate
    assert 'snapshot_fixed_path_fixture_roots "${repo_root}" "fixed-checked-in"' in gate
    checked = gate.index('"fixed-checked-in"')
    first = gate.index('"fixed-pass-1"', checked)
    second = gate.index('"fixed-pass-2"', first)
    assert checked < first < second
    assert "checked-in fixed-path fixture outputs are stale" in gate
    assert "two isolated fixed-path fixture regenerations produced different bytes" in gate
    assert 'if ! cmp -s "${first}" "${second}"' in gate


def test_fixture_replays_pin_lockfile_and_reject_missing_tools_or_final_drift() -> None:
    """The release gate must preserve its source and never downgrade prerequisites."""

    gate = fixture_gate()
    assert "the pinned root Cargo.lock must be a regular file" in gate
    assert 'cp "${repo_root}/Cargo.lock" "${worktree}/Cargo.lock"' in gate
    assert "isolated replay Cargo.lock copy changed bytes" in gate
    assert gate.count("require_clean_checkout") >= 3
    assert 'require_fixture_tool node "SF1 vector parity"' in gate
    assert 'require_fixture_tool go "1 GiB Go regression"' in gate
    assert "skipping ${check_label}" not in gate
