"""Tests for the release source-manifest helper."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "compute_workspace_source_manifest.py"


def load_module():
    spec = importlib.util.spec_from_file_location("workspace_source_manifest", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def init_release_repo(path: Path) -> None:
    subprocess.run(["git", "init", "-q"], cwd=path, check=True)
    subprocess.run(
        ["git", "config", "user.email", "release-test@example.invalid"],
        cwd=path,
        check=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Release Test"], cwd=path, check=True
    )
    (path / ".gitignore").write_text("Cargo.lock\ntarget/\n", encoding="utf-8")
    (path / "tracked.txt").write_text("source\n", encoding="utf-8")
    subprocess.run(
        ["git", "add", ".gitignore", "tracked.txt"], cwd=path, check=True
    )
    subprocess.run(
        ["git", "commit", "-qm", "fixture"], cwd=path, check=True
    )
    (path / "Cargo.lock").write_text("version = 3\n", encoding="utf-8")


def test_manifest_is_order_independent_and_content_sensitive(tmp_path: Path) -> None:
    module = load_module()
    (tmp_path / "a.txt").write_text("alpha\n", encoding="utf-8")
    (tmp_path / "b.txt").write_text("beta\n", encoding="utf-8")

    first = module._manifest_for_paths(tmp_path, ["b.txt", "a.txt"])
    assert first == module._manifest_for_paths(tmp_path, ["a.txt", "b.txt"])

    (tmp_path / "a.txt").write_text("changed\n", encoding="utf-8")
    assert first != module._manifest_for_paths(tmp_path, ["a.txt", "b.txt"])


def test_manifest_distinguishes_deleted_and_symlink_entries(tmp_path: Path) -> None:
    module = load_module()
    (tmp_path / "target-a").write_text("same\n", encoding="utf-8")
    (tmp_path / "target-b").write_text("same\n", encoding="utf-8")
    (tmp_path / "link").symlink_to("target-a")

    first = module._manifest_for_paths(tmp_path, ["link", "missing"])
    (tmp_path / "link").unlink()
    (tmp_path / "link").symlink_to("target-b")
    second = module._manifest_for_paths(tmp_path, ["link", "missing"])
    assert first != second

    (tmp_path / "missing").write_text("now present\n", encoding="utf-8")
    assert second != module._manifest_for_paths(tmp_path, ["link", "missing"])


def test_manifest_tracks_executable_mode(tmp_path: Path) -> None:
    module = load_module()
    script = tmp_path / "gate.sh"
    script.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    script.chmod(0o644)
    regular = module._manifest_for_paths(tmp_path, ["gate.sh"])
    script.chmod(0o755)
    executable = module._manifest_for_paths(tmp_path, ["gate.sh"])
    assert regular != executable
    assert os.access(script, os.X_OK)


def test_workspace_manifest_binds_ignored_cargo_lock(tmp_path: Path) -> None:
    module = load_module()
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / ".gitignore").write_text("Cargo.lock\n", encoding="utf-8")
    (tmp_path / "tracked.txt").write_text("source\n", encoding="utf-8")
    subprocess.run(
        ["git", "add", ".gitignore", "tracked.txt"],
        cwd=tmp_path,
        check=True,
    )

    lockfile = tmp_path / "Cargo.lock"
    lockfile.write_text("version = 3\n", encoding="utf-8")
    assert "Cargo.lock" in module._git_source_paths(tmp_path)
    first = module.workspace_source_manifest(tmp_path)

    lockfile.write_text("version = 4\n", encoding="utf-8")
    assert first != module.workspace_source_manifest(tmp_path)


def test_git_unmerged_paths_are_parsed_and_deduplicated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    output = (
        b"100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tconflict.rs\0"
        b"100644 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 2\tconflict.rs\0"
        b"100644 cccccccccccccccccccccccccccccccccccccccc 3\tdocs/note.md\0"
    )

    def fake_run(*_args, **_kwargs):
        return subprocess.CompletedProcess([], 0, stdout=output)

    monkeypatch.setattr(module.subprocess, "run", fake_run)
    assert module._git_unmerged_paths(tmp_path) == [
        "conflict.rs",
        "docs/note.md",
    ]


def test_workspace_manifest_rejects_unmerged_index(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_module()
    monkeypatch.setattr(
        module,
        "_git_unmerged_paths",
        lambda _root: ["conflict.rs", "docs/note.md"],
    )

    with pytest.raises(
        module.UnmergedSourceError,
        match=r"unresolved merge entries: conflict\.rs, docs/note\.md",
    ):
        module._git_source_paths(tmp_path)


@pytest.mark.parametrize(
    ("label", "git_path", "directory"),
    [
        ("merge", "MERGE_HEAD", False),
        ("cherry-pick", "CHERRY_PICK_HEAD", False),
        ("revert", "REVERT_HEAD", False),
        ("mailbox apply", "AM_HEAD", False),
        ("rebase-apply", "rebase-apply", True),
        ("rebase-merge", "rebase-merge", True),
        ("sequencer", "sequencer", True),
        ("bisect", "BISECT_START", False),
    ],
)
def test_workspace_manifest_rejects_active_git_operations(
    tmp_path: Path, label: str, git_path: str, directory: bool
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    marker = module._git_path(tmp_path, git_path)
    if label == "bisect":
        marker.symlink_to(marker.parent / "missing-bisect-state")
    elif directory:
        marker.mkdir(parents=True)
    else:
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text("active\n", encoding="utf-8")

    with pytest.raises(module.ActiveGitOperationError, match=label):
        module.workspace_source_manifest(tmp_path)


def test_active_operation_detection_is_linked_worktree_local(tmp_path: Path) -> None:
    module = load_module()
    main = tmp_path / "main"
    linked = tmp_path / "linked"
    main.mkdir()
    init_release_repo(main)
    subprocess.run(
        ["git", "worktree", "add", "--detach", str(linked), "HEAD"],
        cwd=main,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    (linked / "Cargo.lock").write_text("version = 3\n", encoding="utf-8")

    main_marker = module._git_path(main, "MERGE_HEAD")
    main_marker.write_text("active\n", encoding="utf-8")
    assert module._active_git_operations(linked) == []
    module.workspace_source_manifest(linked)

    linked_marker = module._git_path(linked, "MERGE_HEAD")
    linked_marker.write_text("active\n", encoding="utf-8")
    with pytest.raises(module.ActiveGitOperationError, match="merge"):
        module.workspace_source_manifest(linked)


def test_workspace_manifest_rejects_resolved_but_uncommitted_merge(
    tmp_path: Path,
) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    original_branch = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=tmp_path,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    subprocess.run(["git", "switch", "-qc", "merge-side"], cwd=tmp_path, check=True)
    (tmp_path / "tracked.txt").write_text("merge side\n", encoding="utf-8")
    subprocess.run(["git", "commit", "-qam", "merge side"], cwd=tmp_path, check=True)
    subprocess.run(["git", "switch", "-q", original_branch], cwd=tmp_path, check=True)
    (tmp_path / "tracked.txt").write_text("main side\n", encoding="utf-8")
    subprocess.run(["git", "commit", "-qam", "main side"], cwd=tmp_path, check=True)
    merge = subprocess.run(
        ["git", "merge", "merge-side"],
        cwd=tmp_path,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert merge.returncode != 0
    (tmp_path / "tracked.txt").write_text("resolved\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=tmp_path, check=True)
    assert module._git_unmerged_paths(tmp_path) == []

    with pytest.raises(module.ActiveGitOperationError, match="merge"):
        module.workspace_source_manifest(tmp_path)


def test_release_identity_binds_clean_head_tree_manifest_and_lock(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)

    identity = module.release_source_identity(tmp_path)
    assert identity["head_tree"] == identity["index_tree"]
    assert identity["workspace_source_manifest_sha256"] == module.workspace_source_manifest(
        tmp_path
    )
    assert len(identity["head_commit"]) == 40
    assert len(identity["cargo_lock_sha256"]) == 64

    (tmp_path / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
    changed = module.release_source_identity(tmp_path)
    assert changed["head_commit"] == identity["head_commit"]
    assert changed["head_tree"] == identity["head_tree"]
    assert changed["cargo_lock_sha256"] != identity["cargo_lock_sha256"]
    assert (
        changed["workspace_source_manifest_sha256"]
        != identity["workspace_source_manifest_sha256"]
    )


def test_release_identity_rejects_staged_source(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "tracked.txt").write_text("staged\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=tmp_path, check=True)

    with pytest.raises(module.DirtyReleaseSourceError, match="index is not HEAD"):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_tracked_worktree_drift(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "tracked.txt").write_text("dirty\n", encoding="utf-8")

    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_nonignored_untracked_source(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    (tmp_path / "untracked.rs").write_text("fn injected() {}\n", encoding="utf-8")

    with pytest.raises(
        module.DirtyReleaseSourceError, match="non-ignored untracked paths"
    ):
        module.release_source_identity(tmp_path)


def test_release_identity_rejects_missing_or_symlinked_lockfile(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    lockfile = tmp_path / "Cargo.lock"
    lockfile.unlink()
    with pytest.raises(module.DirtyReleaseSourceError, match="regular workspace Cargo.lock"):
        module.release_source_identity(tmp_path)

    ignored_target = tmp_path / "target" / "lock-target"
    ignored_target.parent.mkdir()
    ignored_target.write_text("version = 3\n", encoding="utf-8")
    lockfile.symlink_to("target/lock-target")
    with pytest.raises(module.DirtyReleaseSourceError, match="regular workspace Cargo.lock"):
        module.release_source_identity(tmp_path)


def test_release_identity_detects_same_tree_head_change(tmp_path: Path) -> None:
    module = load_module()
    init_release_repo(tmp_path)
    before = module.release_source_identity(tmp_path)
    subprocess.run(
        ["git", "commit", "--allow-empty", "-qm", "same tree, different release"],
        cwd=tmp_path,
        check=True,
    )
    after = module.release_source_identity(tmp_path)

    assert after["head_commit"] != before["head_commit"]
    assert after["head_tree"] == before["head_tree"]
    assert (
        after["workspace_source_manifest_sha256"]
        == before["workspace_source_manifest_sha256"]
    )
