"""Physical absent-leaf controls for the Node runtime input graph."""
from __future__ import annotations

import os
from pathlib import Path
import hashlib
import struct
import sys
import tempfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import sorafs_javascript_runtime_custody as custody
import sorafs_javascript_runtime_inputs as inputs
from sorafs_javascript_runtime_custody import HeldRuntimeAbsentLeaf, HeldRuntimeAlias, RuntimeInputError
from release_manifest_signing import ReleaseManifestSignatureError
from sorafs_javascript_archive import ArchiveError


@pytest.fixture
def tmp_path():
    """Keep every physical test artifact inside the sole working checkout."""
    target = ROOT / "target"
    target.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="runtime-custody-", dir=target) as path:
        yield Path(path)


def test_original_absent_leaf_is_one_shot_and_refuses_late_file(tmp_path):
    parent = tmp_path / "runtime"
    parent.mkdir()
    path = parent / "missing.dylib"
    owner = HeldRuntimeAbsentLeaf(path)
    owner.recheck()
    path.write_bytes(b"unexpected image")
    with pytest.raises(RuntimeInputError, match="lineage changed|became present"):
        owner.recheck()
    with pytest.raises(RuntimeInputError, match="inactive"):
        owner.recheck()
    owner.close()
    owner.close()


def test_dangling_symlink_is_present_and_never_claimed_as_absence(tmp_path):
    parent = tmp_path / "runtime"
    parent.mkdir()
    path = parent / "missing.dylib"
    path.symlink_to("elsewhere.dylib")
    with pytest.raises(RuntimeInputError, match="became present"):
        HeldRuntimeAbsentLeaf(path)

    actual = tmp_path / "actual"
    actual.mkdir()
    linked = tmp_path / "linked"
    linked.symlink_to(actual, target_is_directory=True)
    with pytest.raises(ReleaseManifestSignatureError, match="without links"):
        HeldRuntimeAbsentLeaf(linked / "missing.dylib")


def test_original_parent_replacement_and_writable_parent_refuse(tmp_path):
    parent = tmp_path / "runtime"
    child = parent / "lib"
    child.mkdir(parents=True)
    path = child / "missing.dylib"
    owner = HeldRuntimeAbsentLeaf(path)
    moved = tmp_path / "runtime-moved"
    parent.rename(moved)
    (parent / "lib").mkdir(parents=True)
    with pytest.raises(RuntimeInputError, match="lineage changed"):
        owner.recheck()
    owner.close()

    writable = tmp_path / "writable"
    writable.mkdir()
    os.chmod(writable, 0o777)
    try:
        with pytest.raises(RuntimeInputError, match="parent policy"):
            HeldRuntimeAbsentLeaf(writable / "missing.dylib")
    finally:
        os.chmod(writable, 0o700)


def test_original_alias_target_is_held_without_following_symlink(tmp_path):
    parent = tmp_path / "runtime"
    parent.mkdir()
    alias = parent / "current"
    alias.symlink_to("release-1", target_is_directory=True)
    owner = HeldRuntimeAlias(alias, "release-1")
    owner.recheck()
    alias.unlink()
    alias.symlink_to("release-2", target_is_directory=True)
    with pytest.raises(RuntimeInputError, match="lineage changed|target differs"):
        owner.recheck()
    owner.close()

    absent = parent / "absent-alias"
    with pytest.raises(RuntimeInputError, match="disappeared"):
        HeldRuntimeAlias(absent, "release-1")


def _one_image_runtime(tmp_path):
    """Build one inert, parseable Mach-O original at its actual test pathname."""
    executable = tmp_path / "runtime" / "node"
    executable.parent.mkdir()
    name = b"/usr/lib/dyld\0"
    size = (12 + len(name) + 7) & ~7
    command = struct.pack("<3I", 14, size, 12) + name + bytes(size - 12 - len(name))
    image = struct.pack("<8I", 0xFEEDFACF, 0x0100000C, 0, 2, 1,
                        len(command), 0, 0) + command
    executable.write_bytes(image)
    executable.chmod(0o755)
    selected = tmp_path / "selected-node"
    selected.symlink_to("runtime/node")
    row = {
        "schema": inputs.SCHEMA, "platform": "darwin", "architecture": "arm64",
        "version": "24.21.0", "selected_executable": str(selected),
        "executable": str(executable), "images": [{
            "path": str(executable), "sha256": hashlib.sha256(image).hexdigest(),
            "size": len(image), "mode": 0o755,
        }], "aliases": [{"path": str(selected), "target": "runtime/node",
                        "resolved": str(executable)}], "edges": [],
    }
    manifest = inputs.canonical_json(row)
    bundle = (inputs.MAGIC + struct.pack(">Q", len(manifest)) + manifest + image)
    return executable, selected, bundle, hashlib.sha256(manifest).hexdigest()


def test_complete_original_runtime_inputs_hold_image_and_alias_until_close(tmp_path):
    executable, selected, bundle, pin = _one_image_runtime(tmp_path)
    with custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin) as owner:
        assert owner.manifest.executable == str(executable)
        assert os.fstat(owner.executable_descriptor).st_size == executable.stat().st_size
        owner.recheck()
        assert selected.is_symlink()
        with pytest.raises(RuntimeInputError, match="one-shot"):
            owner.__enter__()
    with pytest.raises(RuntimeInputError, match="inactive"):
        owner.recheck()

    changed = custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin)
    executable.write_bytes(b"changed")
    with pytest.raises(Exception, match="original|changed"):
        changed.recheck()
    changed.close()


def test_runtime_physical_handle_budget_refuses_before_acquisition(tmp_path, monkeypatch):
    _executable, _selected, bundle, pin = _one_image_runtime(tmp_path)
    manifest = inputs.parse_node_runtime_bundle(
        bundle, expected_manifest_sha256=pin).manifest
    paths = [row.path for row in manifest.images]
    paths.extend(row.path for row in manifest.aliases)
    peak = sum(len(Path(row.path).parts) for row in manifest.images)
    peak += sum(len(Path(row.path).parts) - 1 for row in manifest.aliases)
    peak += max(len(Path(path).parts) - 1 for path in paths)
    monkeypatch.setattr(custody, "MAX_RUNTIME_HELD_DESCRIPTORS", peak)
    with custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin):
        pass
    monkeypatch.setattr(custody, "MAX_RUNTIME_HELD_DESCRIPTORS", peak - 1)
    def forbidden(*_args, **_kwargs):
        raise AssertionError("physical acquisition occurred before descriptor admission")
    monkeypatch.setattr(custody, "HeldInputFile", forbidden)
    with pytest.raises(RuntimeInputError, match="descriptor admission"):
        custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin)


def test_constructor_failure_after_image_acquisition_closes_original(tmp_path, monkeypatch):
    _executable, selected, bundle, pin = _one_image_runtime(tmp_path)
    original = custody.HeldInputFile
    opened = []

    def record_image(*args, **kwargs):
        owner = original(*args, **kwargs)
        opened.append(owner.descriptor)
        return owner

    monkeypatch.setattr(custody, "HeldInputFile", record_image)
    selected.unlink()
    selected.symlink_to("foreign-node")
    with pytest.raises(RuntimeInputError, match="target differs"):
        custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin)
    assert len(opened) == 1
    with pytest.raises(OSError):
        os.fstat(opened[0])


def test_failed_scope_entry_closes_every_acquired_original(tmp_path):
    executable, _selected, bundle, pin = _one_image_runtime(tmp_path)
    owner = custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin)
    original_fd = owner.executable_descriptor
    executable.write_bytes(b"changed after acquisition")
    with pytest.raises(ArchiveError, match="changed"):
        with owner:
            pytest.fail("a changed original cannot enter the execution scope")
    with pytest.raises(OSError):
        os.fstat(original_fd)
    with pytest.raises(RuntimeInputError, match="inactive"):
        owner.recheck()


def test_scope_exit_detects_alias_change_and_closes_originals(tmp_path):
    _executable, selected, bundle, pin = _one_image_runtime(tmp_path)
    owner = custody.OriginalNodeRuntimeInputs(bundle, expected_manifest_sha256=pin)
    original_fd = owner.executable_descriptor
    with pytest.raises(RuntimeInputError, match="lineage changed|target differs"):
        with owner:
            selected.unlink()
            selected.symlink_to("foreign-node")
    with pytest.raises(OSError):
        os.fstat(original_fd)
    with pytest.raises(RuntimeInputError, match="inactive"):
        owner.recheck()
