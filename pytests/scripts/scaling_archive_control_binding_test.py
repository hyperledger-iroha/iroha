"""Current V1 control paths remain original even when content hashes agree."""

import json
import os
import shutil

import pytest

import scaling_archive_data as archive
import scaling_experiment_files as files
from scaling_archive_data_test import complete, put, refresh_public_hashes
from scaling_experiment_files_test import case, complete as publish_controls, failed


@pytest.mark.parametrize('substitution', ('other_role', 'other_run'))
def test_rehashed_equal_content_cannot_replace_original_artifact_path(
    complete, tmp_path, substitution,
):
    """A receipt's exact role and run path survives an ordinary hash refresh."""
    source, plan, budget, _, _ = complete
    root = tmp_path / 'evidence'
    shutil.copytree(source, root)
    first = root / 'runs/pair-01/one_lane/run_receipt.json'
    receipt = json.loads(first.read_bytes())
    original = next(row for row in receipt['artifacts'] if row['role'] == 'native_finality')
    if substitution == 'other_role':
        other = next(row for row in receipt['artifacts'] if row['role'] == 'native_queries')
        put(root / other['path'], (root / original['path']).read_bytes())
    else:
        second = json.loads((root / 'runs/pair-02/one_lane/run_receipt.json').read_bytes())
        other = next(row for row in second['artifacts'] if row['role'] == 'native_finality')
    expected = refresh_public_hashes(root)
    assert archive.inspect_archive(root, plan, budget, expected).inventory.sha256 == expected.inventory_sha256
    receipt = json.loads(first.read_bytes())
    original = next(row for row in receipt['artifacts'] if row['role'] == 'native_finality')
    if substitution == 'other_role':
        other = next(row for row in receipt['artifacts'] if row['role'] == 'native_queries')
    assert original['path'] != other['path']
    assert original['sha256'] == other['sha256']
    original['path'] = other['path']
    put(first, archive.encode(receipt))
    expected = refresh_public_hashes(root)
    with pytest.raises(archive.ArchiveDataError):
        archive.inspect_archive(root, plan, budget, expected)


def test_original_control_owner_rejects_same_byte_symlink_replacement_before_read(
    case, monkeypatch,
):
    """An admitted descriptor cannot be redirected to new path custody."""
    bindings = publish_controls(case)
    original = bindings[0]
    path = case.root / original.path
    replacement = case.root.parent / 'same-bytes'
    replacement.write_bytes(path.read_bytes())
    replacement.chmod(0o600)
    path.unlink()
    path.symlink_to(replacement)
    reads = []
    original_read = os.pread

    def reading(*args):
        reads.append(args)
        return original_read(*args)

    monkeypatch.setattr(files.os, 'pread', reading)
    with pytest.raises(files.ExperimentFileError):
        case.owner.read_control(original, max_bytes=len(replacement.read_bytes()))
    assert reads == []
    assert replacement.read_bytes() == b'{"identity":1}\n'
    failed(case.owner)


@pytest.mark.parametrize('invalid_cap', (False, -1, None, 1.0, 'over_cap'))
def test_original_control_owner_rejects_invalid_semantic_cap_before_read(
    case, monkeypatch, invalid_cap,
):
    """A borrowed control cannot widen or mistype its original byte charge."""
    binding = publish_controls(case)[0]
    maximum = case.owner._caps[binding.label]
    cap = maximum + 1 if invalid_cap == 'over_cap' else invalid_cap
    reads = []
    original_read = os.pread

    def reading(*args):
        reads.append(args)
        return original_read(*args)

    monkeypatch.setattr(files.os, 'pread', reading)
    with pytest.raises(files.ExperimentFileError):
        case.owner.read_control(binding, max_bytes=cap)
    assert reads == []
    failed(case.owner)
