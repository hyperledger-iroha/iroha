"""Contracts for full-tree deterministic SoraFS fixture regeneration."""

from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def fixture_gate() -> str:
    """Return the fixture release gate source."""

    return (REPO_ROOT / "ci/check_sorafs_fixtures.sh").read_text(encoding="utf-8")


def sf1_owner() -> str:
    """Return the Rust-owned SF1 generator source."""

    return (
        REPO_ROOT / "crates/sorafs_chunker/src/bin/export_vectors.rs"
    ).read_text(encoding="utf-8")


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
    assert 'local sf1_stage="${fixture_snapshot_root}/sf1-stage-${worktree##*/}"' in fixed_runner
    assert 'mkdir -m 0700 -- "${sf1_stage}"' in fixed_runner
    assert '--write --staging-root "${sf1_stage}"' in fixed_runner
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


def test_sf1_owner_stages_validates_and_rolls_back_publication() -> None:
    """SF1 publication must not expose generator or signer failures in place."""

    owner = sf1_owner()
    assert "Usage: export_vectors (--check|--write) --staging-root <path>" in owner
    assert '"--staging-root must be outside the repository"' in owner
    assert '"--staging-root must have exact mode 0700"' in owner
    assert '"--staging-root must be empty"' in owner
    assert "const GENERATED_PATHS: [&str; 8]" in owner
    assert "validate_staged_tree(&staging_root)?;" in owner
    assert "&fuzz_digests" in owner
    assert "staged fuzz output changed after generation" in owner
    assert "check_staged_tree(&staging_root, &repo_root)?" in owner
    assert "publish_staged_tree(&staging_root, &repo_root)?" in owner
    assert "create_private_sibling(&target, \"new\", &generated)" in owner
    assert "create_private_sibling(&target, \"backup\", &original)" in owner
    assert "rollback_publications(publications, committed, None)" in owner
    assert "cleanup_publications(publications, !rollback_errors.is_empty())" in owner
    assert "publication_failure_rolls_back_every_committed_target" in owner
    assert "fs::rename(&publication.replacement, &publication.target)" in owner
    assert "fs::create_dir_all(&output_dir)?;" in owner
    assert 'repo_root.join("fixtures").join("sorafs_chunker")' not in owner


def test_sf1_owner_requires_external_signer_only_for_manifest_drift() -> None:
    """A changed signed manifest cannot be published without explicit authority."""

    owner = sf1_owner()
    assert '"--signing-key-file must be a regular non-symbolic file"' in owner
    assert '"--signing-key-file must not grant group or other permissions"' in owner
    assert "let manifest_changed = blake3::hash(&read_regular_file(&live_manifest)?)" in owner
    assert "if manifest_changed" in owner
    assert "if staged_cli.signing_key_hex.is_none()" in owner
    assert (
        '"manifest digest changed; explicit signing-key authority is required"'
        in owner
    )
    assert "read_regular_file(&live_signature)?" in owner
    assert "MAX_SIGNING_KEY_FILE_BYTES" in owner
    assert "MAX_GENERATED_FILE_BYTES" in owner


def test_sf1_registered_owner_uses_private_stage_and_compiler_free_check() -> None:
    """The registry must expose the guarded writer and exact byte-level check."""

    manifest = (REPO_ROOT / "generated-files.toml").read_text(encoding="utf-8")
    entry = manifest.split('name = "sorafs-sf1-fixtures"', 1)[1].split(
        "[[generated]]", 1
    )[0]
    assert "--offline --jobs 1" in entry
    assert "--write --staging-root" in entry
    assert "IROHA_SF1_FIXTURE_STAGE" in entry
    assert "IROHA_SF1_MANIFEST_SIGNING_KEY_FILE" in entry
    assert "node --test scripts/tests/check_sf1_vectors.test.mjs" in entry
    assert "node scripts/check_sf1_vectors.mjs" in entry
