"""Original-descriptor directory publication controls on the actual local filesystem."""

import hashlib
import os
from pathlib import Path
import tempfile

import pytest

from scripts import release_artifact_contract as contract


@pytest.fixture
def workspace():
    """Own every ancestor used by the directory custody contract."""
    target = Path(__file__).resolve().parents[2] / "target"
    target.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=target, prefix="directory-publication-") as directory:
        yield Path(directory)


@pytest.fixture
def held(workspace):
    parent = workspace / "parent"
    stage = contract.create_fresh_directory(parent / "stage", mode=0o700)
    parent_fd, _, _ = contract._open_absolute_directory(parent, "test parent")
    stage_fd, _, _ = contract._open_absolute_directory(stage, "test stage")
    try:
        yield parent, stage, {"parent_fd": parent_fd, "stage_fd": stage_fd}
    finally:
        os.close(stage_fd)
        os.close(parent_fd)


def test_private_directory_is_published_without_copying_its_original_inode(held):
    parent, stage, descriptors = held
    contract.exclusive_write_bytes(stage / "content", b"qualified candidate bytes", mode=0o600)
    before = stage.stat()
    destination = parent / "published"
    contract.publish_directory_noreplace(stage, destination, **descriptors)
    assert (destination.stat().st_dev, destination.stat().st_ino) == (before.st_dev, before.st_ino)
    assert not stage.exists()
    assert (destination / "content").read_bytes() == b"qualified candidate bytes"


def test_public_source_mode_requires_explicit_exact_admission(held):
    parent, stage, descriptors = held
    stage.chmod(0o755)
    before = stage.stat()
    destination = parent / "published"
    with pytest.raises(contract.ReleaseArtifactError, match="custody changed"):
        contract.publish_directory_noreplace(stage, destination, **descriptors)
    assert stage.stat().st_ino == before.st_ino and not destination.exists()
    contract.publish_directory_noreplace(stage, destination, stage_mode=0o755, **descriptors)
    assert (destination.stat().st_dev, destination.stat().st_ino) == (before.st_dev, before.st_ino)
    assert destination.stat().st_mode & 0o7777 == 0o755


@pytest.mark.parametrize("mode", [0o750, 0o770, 0o777, 0o1700, 0o2700, 0o4700, True])
def test_unreviewed_publication_mode_refuses_before_rename(held, mode):
    parent, stage, descriptors = held
    before = stage.stat()
    destination = parent / "published"
    with pytest.raises(contract.ReleaseArtifactError, match="mode must be exactly"):
        contract.publish_directory_noreplace(stage, destination, stage_mode=mode, **descriptors)
    assert stage.stat().st_ino == before.st_ino and not destination.exists()


@pytest.mark.parametrize("admitted,actual", [(0o700, 0o755), (0o755, 0o700), (0o755, 0o775)])
def test_exact_admitted_mode_cannot_be_substituted(held, admitted, actual):
    parent, stage, descriptors = held
    stage.chmod(actual)
    destination = parent / "published"
    with pytest.raises(contract.ReleaseArtifactError, match="custody changed"):
        contract.publish_directory_noreplace(stage, destination, stage_mode=admitted, **descriptors)
    assert stage.is_dir() and not destination.exists()


@pytest.mark.parametrize("kind", ["empty", "populated", "symlink"])
def test_destination_is_never_replaceable_even_if_empty(held, kind):
    parent, stage, descriptors = held
    destination = parent / "published"
    if kind == "symlink":
        destination.symlink_to(parent / "missing")
    else:
        destination.mkdir()
        if kind == "populated":
            (destination / "sentinel").write_bytes(b"keep")
    before = destination.lstat()
    with pytest.raises(contract.ReleaseArtifactError, match="publication failed"):
        contract.publish_directory_noreplace(stage, destination, **descriptors)
    assert destination.lstat().st_ino == before.st_ino
    assert stage.is_dir()


@pytest.mark.parametrize("replace", ["parent", "stage", "link"])
def test_original_parent_and_stage_descriptors_cannot_be_rebound(held, replace):
    parent, stage, descriptors = held
    if replace == "stage":
        stage.rename(parent / "original")
        stage.mkdir(mode=0o700)
    else:
        moved = parent.with_name("original")
        parent.rename(moved)
        if replace == "link":
            parent.symlink_to(moved, target_is_directory=True)
        else:
            parent.mkdir(mode=0o700)
            stage.mkdir(mode=0o700)
    with pytest.raises(contract.ReleaseArtifactError):
        contract.publish_directory_noreplace(stage, parent / "published", **descriptors)
    assert not (parent / "published").exists()


def test_unsupported_host_never_uses_check_then_rename(held, monkeypatch):
    parent, stage, descriptors = held
    monkeypatch.setattr(contract.sys, "platform", "unsupported")
    with pytest.raises(contract.ReleaseArtifactError, match="requires Linux or macOS"):
        contract.publish_directory_noreplace(stage, parent / "published", **descriptors)
    assert stage.is_dir()


def test_empty_producer_logs_require_explicit_admission(workspace: Path):
    log = workspace / "stderr"
    contract.exclusive_write_bytes(log, b"", mode=0o600)
    with pytest.raises(contract.ReleaseArtifactError, match="must not be empty"):
        contract.stable_hash_path(log)
    info = contract.stable_hash_path(log, max_size=0, allow_empty=True)
    assert info.size == 0 and info.sha256 == hashlib.sha256(b"").hexdigest()
    log.unlink()
    log.symlink_to(workspace / "missing")
    with pytest.raises(contract.ReleaseArtifactError):
        contract.stable_hash_path(log, max_size=0, allow_empty=True)


@pytest.mark.parametrize("collision", [False, True])
def test_taira_delegation_preserves_local_publication_contract(held, monkeypatch, collision):
    # This exercises the actual adapter without inventing Git/GPG source evidence.
    monkeypatch.syspath_prepend(str(Path(__file__).resolve().parents[1]))
    import taira_source_capture as source
    parent, stage, _ = held
    destination = parent / "published"
    before = stage.stat().st_ino
    if collision:
        destination.mkdir(mode=0o700)
        preserved = destination.stat().st_ino
        with pytest.raises(source.SourceCaptureError, match="exclusive source publication failed"):
            source._publish(stage, destination)
        assert destination.stat().st_ino == preserved
        assert stage.stat().st_ino == before
    else:
        source._publish(stage, destination)
        assert destination.stat().st_ino == before


def test_taira_public_source_delegation_retains_original_inode(held, monkeypatch):
    monkeypatch.syspath_prepend(str(Path(__file__).resolve().parents[1]))
    import taira_source_capture as source
    parent, stage, _ = held
    stage.chmod(0o755)
    before = stage.stat().st_ino
    destination = parent / "published"
    source._publish(stage, destination, stage_mode=0o755)
    assert destination.stat().st_ino == before
    assert destination.stat().st_mode & 0o7777 == 0o755


@pytest.mark.parametrize("depth", [1, 2])
def test_tool_ancestor_rename_and_restore_cannot_reuse_unchanged_file_identity(workspace, depth):
    parent = contract.create_fresh_directory(workspace / "one/two", mode=0o700)
    tool = parent / "tool"
    contract.exclusive_write_bytes(tool, b"original executable", mode=0o755)
    before = contract.stable_hash_path(tool)
    selected = tool.parents[depth - 1]
    renamed = selected.with_name("saved")
    with pytest.raises(contract.ReleaseArtifactError, match="ancestor changed"):
        with contract.pin_path_ancestors(tool):
            selected.rename(renamed)
            renamed.rename(selected)
    assert contract.stable_hash_path(tool) == before
