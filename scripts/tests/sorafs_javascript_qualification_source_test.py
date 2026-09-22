"""Closed original-source and real POSIX copied-core controls; no SDK execution."""
from __future__ import annotations

from dataclasses import FrozenInstanceError, replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_qualification_source as source
import sorafs_javascript_qualification_custody as custody
import sorafs_javascript_tree_custody as tree_owner
from sorafs_javascript_archive import ArchiveError

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "scripts/fixtures/sorafs_javascript_qualification_sources_v1.json"


@pytest.fixture(scope="module")
def originals():
    raw = CATALOG.read_bytes()
    catalog = json.loads(raw)
    names = [row["path"] for row in catalog["source_files"]] + catalog["fixture_files"]
    return raw, {name: (ROOT / name).read_bytes() for name in names}


@pytest.fixture(scope="module")
def projection(originals):
    raw, sources = originals
    return source.qualification_projection(sources, catalog=raw)


@pytest.fixture
def copied(tmp_path, projection):
    root = tmp_path / "qualification-core"
    root.mkdir(mode=0o700)
    for row in projection.members:
        path = root / row.path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(row.content)
        path.chmod(0o644)
    return root.resolve()


def test_catalog_closes_actual_source_and_complete_fixture_census(originals, projection):
    raw, sources = originals
    catalog = json.loads(raw)
    actual = set()
    for prefix in source.FIXTURE_PREFIXES:
        for path in (ROOT / prefix).rglob("*"):
            assert not path.is_symlink()
            if path.is_file():
                actual.add(path.relative_to(ROOT).as_posix())
    actual.add(source.PAYLOAD)
    assert actual == set(catalog["fixture_files"])
    assert len(actual) == 182 and len(catalog["source_files"]) == 9
    assert len(projection.members) == 191
    assert sum(len(row.content) for row in projection.members) == sum(map(len, sources.values()))
    assert [row.path for row in projection.members] == sorted(sources)
    contract = json.loads(sources[source.CONTRACT])
    assert sum(row["assertion_count"] for row in contract["suites"]) == 172
    assert sum(len(row["cases"]) for row in contract["suites"]) == 46
    assert sum(len(row["nested_case_names"]) for row in contract["suites"]) == 9
    assert all(not row.path.startswith("javascript/iroha_js/src/") for row in projection.members)


def test_pure_projection_never_opens_files_or_starts_a_process(originals, projection, monkeypatch):
    def forbidden(*_args, **_kwargs):
        raise AssertionError("pure relation attempted I/O")
    raw, sources = originals
    with monkeypatch.context() as patch:
        patch.setattr("builtins.open", forbidden)
        patch.setattr(Path, "open", forbidden)
        patch.setattr(os, "open", forbidden)
        patch.setattr(subprocess, "Popen", forbidden)
        assert source.qualification_projection(sources, catalog=raw) == projection
        source.validate_qualification_projection(projection)
        source.verify_qualification_content(projection, files=sources,
                                            modes={name: 0o644 for name in sources})


def test_frozen_projection_retains_source_bytes_not_mutable_caller_dictionary(originals, projection):
    raw, sources = originals
    supplied = dict(sources)
    captured = source.qualification_projection(supplied, catalog=raw)
    supplied.clear()
    assert captured == projection
    with pytest.raises(FrozenInstanceError):
        captured.members = ()
    with pytest.raises(FrozenInstanceError):
        captured.members[0].content = b"changed"


@pytest.mark.parametrize("kind", ("truncated", "appended", "mutable", "oversized", "reselected"))
def test_untrusted_catalog_cannot_choose_a_different_closed_corpus(originals, kind):
    raw, sources = originals
    if kind == "truncated": raw = raw[:-1]
    if kind == "appended": raw += b" "
    if kind == "mutable": raw = bytearray(raw)
    if kind == "oversized": raw = b"x" * (source.MAX_CATALOG_BYTES + 1)
    if kind == "reselected":
        value = json.loads(raw); value["fixture_files"][0] = "foreign/file"; raw = json.dumps(value).encode()
    with pytest.raises(ArchiveError, match="catalog"):
        source.qualification_projection(sources, catalog=raw)


@pytest.mark.parametrize("index", range(9))
def test_every_executable_or_module_scope_original_is_byte_bound(originals, index):
    raw, sources = originals
    name = json.loads(raw)["source_files"][index]["path"]
    changed = dict(sources); changed[name] += b" "
    with pytest.raises(ArchiveError, match="shared code or module metadata"):
        source.qualification_projection(changed, catalog=raw)


@pytest.mark.parametrize("kind", ("suite", "helper", "contract", "package", "fixture", "payload", "metadata"))
def test_missing_original_is_never_an_empty_or_partial_success(originals, kind):
    raw, sources = originals
    candidates = {
        "suite": next(name for name in sources if "/sorafsNativeSuites/" in name),
        "helper": "javascript/iroha_js/test/helpers/nativeRequirements.js",
        "contract": source.CONTRACT, "package": source.PACKAGE,
        "fixture": json.loads(raw)["fixture_files"][0], "payload": source.PAYLOAD,
        "metadata": source.METADATA,
    }
    changed = dict(sources); del changed[candidates[kind]]
    with pytest.raises(ArchiveError, match="inventory"):
        source.qualification_projection(changed, catalog=raw)


@pytest.mark.parametrize("replacement", (
    "javascript/iroha_js/src/native.js", "javascript/iroha_js/test/helpers/native.js",
    "javascript/iroha_js/test/package.json", "node_modules/unowned/index.js",
    "fixtures/.npmrc", "fixtures/sorafs_manifest/../escape", "Fixtures/sorafs_manifest/extra",
    "fixtures/sorafs_manifest/extra", "fixtures/sorafs_manifest/alias\\name", "\0",
))
def test_same_size_dictionary_cannot_substitute_selectors_implementation_or_aliases(originals, replacement):
    raw, sources = originals
    changed = dict(sources); value = changed.pop(json.loads(raw)["fixture_files"][0]); changed[replacement] = value
    with pytest.raises(ArchiveError, match="census"):
        source.qualification_projection(changed, catalog=raw)


@pytest.mark.parametrize("value", ("../outside", "/outside", "fuzz/../outside", "", None, [], 1,
    "fuzz/sorafs_chunker/sf1_profile_v1_input.bin?alias"))
def test_metadata_cannot_choose_any_other_payload(originals, value):
    raw, sources = originals
    changed = dict(sources); metadata = json.loads(changed[source.METADATA]); metadata["payload_path"] = value
    changed[source.METADATA] = json.dumps(metadata).encode()
    with pytest.raises(ArchiveError, match="payload"):
        source.qualification_projection(changed, catalog=raw)


@pytest.mark.parametrize("value", (True, 1.0, -1, 0, 1, "1048576", None))
def test_metadata_payload_length_is_exact_and_integer(originals, value):
    raw, sources = originals
    changed = dict(sources); metadata = json.loads(changed[source.METADATA]); metadata["payload_bytes"] = value
    changed[source.METADATA] = json.dumps(metadata).encode()
    with pytest.raises(ArchiveError, match="payload"):
        source.qualification_projection(changed, catalog=raw)


def test_duplicate_metadata_fields_refuse_before_a_payload_selection(originals):
    raw, sources = originals
    changed = dict(sources)
    changed[source.METADATA] = b'{"payload_path":"escape","payload_path":"' + source.PAYLOAD.encode() + b'","payload_bytes":1048576}'
    with pytest.raises(ArchiveError): source.qualification_projection(changed, catalog=raw)


def test_fixture_bytes_are_candidate_inputs_not_self_issued_release_authority(originals):
    raw, sources = originals
    changed = dict(sources)
    name = next(name for name in sources if name.endswith("README.md"))
    changed[name] = b"independently selected candidate fixture bytes"
    projection = source.qualification_projection(changed, catalog=raw)
    assert next(row.content for row in projection.members if row.path == name) == changed[name]


@pytest.mark.parametrize("kind", ("member_list", "row_type", "duplicate", "reverse", "mutable", "long_name", "changed_code"))
def test_forged_projection_cannot_bypass_original_rederivation(projection, kind):
    members = list(projection.members)
    if kind == "member_list": changed = replace(projection, members=members)
    else:
        if kind == "row_type": members[0] = (members[0].path, members[0].content)
        if kind == "duplicate": members[0] = members[1]
        if kind == "reverse": members.reverse()
        if kind == "mutable": members[0] = replace(members[0], content=bytearray(members[0].content))
        if kind == "long_name": members[0] = replace(members[0], path="x" * 1025)
        if kind == "changed_code":
            index = next(i for i,row in enumerate(members) if "/sorafsNativeSuites/" in row.path)
            members[index] = replace(members[index], content=b"substituted test body")
        changed = replace(projection, members=tuple(members))
    with pytest.raises(ArchiveError): source.validate_qualification_projection(changed)


@pytest.mark.parametrize("kind", ("count", "name", "content", "mode", "float_mode", "extra_mode"))
def test_captured_core_requires_every_exact_original_byte_and_mode(originals, projection, kind):
    _raw, sources = originals
    files = dict(sources); modes = {name: 0o644 for name in sources}; first = next(iter(files))
    if kind == "count": files.pop(first)
    if kind == "name": files["foreign"] = files.pop(first)
    if kind == "content": files[first] += b"changed"
    if kind == "mode": modes[first] = 0o755
    if kind == "float_mode": modes[first] = float(0o644)
    if kind == "extra_mode": modes["foreign"] = 0o644
    with pytest.raises(ArchiveError): source.verify_qualification_content(projection, files=files, modes=modes)


@pytest.mark.parametrize("limit", ("MAX_SOURCE_BYTES", "MAX_SOURCE_FILE_BYTES"))
def test_exact_content_capacity_and_one_below(originals, projection, monkeypatch, limit):
    raw, sources = originals
    value = sum(map(len, sources.values())) if limit == "MAX_SOURCE_BYTES" else max(map(len, sources.values()))
    monkeypatch.setattr(source, limit, value)
    assert source.qualification_projection(sources, catalog=raw) == projection
    monkeypatch.setattr(source, limit, value - 1)
    with pytest.raises(ArchiveError): source.qualification_projection(sources, catalog=raw)


def test_cardinality_refuses_before_set_enumeration(originals, monkeypatch):
    raw, sources = originals
    changed = dict(sources); changed["extra"] = b""
    def forbidden(*_args): raise AssertionError("oversized dictionary reached set")
    monkeypatch.setattr(source, "set", forbidden, raising=False)
    with pytest.raises(ArchiveError, match="inventory"): source.qualification_projection(changed, catalog=raw)


def test_original_tree_keeps_same_owner_and_closes_without_removing_inputs(copied, projection):
    owner = custody.OriginalQualificationTree(copied, projection)
    with owner:
        handles = owner._parent[2]
        assert owner.root == copied
        assert len(owner._state) > 191
        owner.recheck(); owner.recheck()
        for fd in handles: os.fstat(fd)
    for fd in handles:
        with pytest.raises(OSError): os.fstat(fd)
    assert (copied / source.PAYLOAD).is_file()
    with pytest.raises(ArchiveError): owner.recheck()
    with pytest.raises(ArchiveError): owner.__enter__()


@pytest.mark.parametrize("change", ("missing", "source_extra", "metadata_extra", "empty_directory", "mode", "symlink", "hardlink", "fifo", "fixture_bytes"))
def test_live_copied_core_cannot_substitute_implementation_selectors_or_physical_inputs(copied, projection, change):
    leaf = copied / source.PAYLOAD
    if change == "missing": leaf.unlink()
    if change in ("source_extra", "metadata_extra"):
        extra = copied / ("javascript/iroha_js/src/native.js" if change == "source_extra" else "javascript/iroha_js/test/package.json")
        extra.parent.mkdir(parents=True, exist_ok=True); extra.write_bytes(b"{}")
    if change == "empty_directory": (copied / "foreign").mkdir()
    if change == "mode": leaf.chmod(0o755)
    if change == "symlink": leaf.unlink(); leaf.symlink_to(copied / source.METADATA)
    if change == "hardlink": os.link(leaf, copied.parent / "other-owner")
    if change == "fifo": leaf.unlink(); os.mkfifo(leaf)
    if change == "fixture_bytes": leaf.write_bytes(b"substituted")
    owner = custody.OriginalQualificationTree(copied, projection)
    with pytest.raises((ArchiveError, OSError)): owner.__enter__()
    assert owner._parent is None and owner._closed and owner._failed


def test_retained_core_cannot_follow_a_recreated_root(copied, projection):
    owner = custody.OriginalQualificationTree(copied, projection).__enter__()
    try:
        moved = copied.with_name("original-held-core"); copied.rename(moved)
        shutil.copytree(moved, copied)
        with pytest.raises(ArchiveError): owner.recheck()
        with pytest.raises(ArchiveError, match="previously failed"): owner.recheck()
    finally: owner.close()


def test_source_wrapper_rejects_untyped_projection_before_filesystem_io(copied, monkeypatch):
    def forbidden(*_args, **_kwargs): raise AssertionError("unvalidated source reached physical open")
    monkeypatch.setattr(os, "open", forbidden)
    with pytest.raises(ArchiveError): custody.OriginalQualificationTree(copied, object())


@pytest.mark.parametrize("kind", ("list", "wrong_row", "mutable", "float_mode", "traversal", "duplicate", "case_alias", "ancestor", "depth", "long_name"))
def test_shared_physical_kernel_bounds_immutable_expectations_before_open(tmp_path, kind, monkeypatch):
    members = (tree_owner.TreeMember("x", b"bytes", 0o644),)
    if kind == "list": members = list(members)
    if kind == "wrong_row": members = (("x", b"bytes", 0o644),)
    if kind == "mutable": members = (replace(members[0], content=bytearray(b"bytes")),)
    if kind == "float_mode": members = (replace(members[0], mode=float(0o644)),)
    if kind == "traversal": members = (replace(members[0], path="../x"),)
    if kind == "duplicate": members *= 2
    if kind == "case_alias": members += (replace(members[0], path="X"),)
    if kind == "ancestor": members += (replace(members[0], path="x/y"),)
    if kind == "depth": members = (replace(members[0], path="x/" * 64 + "leaf"),)
    if kind == "long_name": members = (replace(members[0], path="x" * 1025),)
    def forbidden(*_args, **_kwargs): raise AssertionError("invalid expectations reached filesystem")
    monkeypatch.setattr(os, "open", forbidden)
    with pytest.raises(ArchiveError): tree_owner.OriginalTree(tmp_path, members)


@pytest.mark.parametrize("limit", ("MAX_TREE_FILES", "MAX_TREE_BYTES", "MAX_TREE_FILE_BYTES"))
def test_shared_kernel_exact_capacity_and_one_below(tmp_path, monkeypatch, limit):
    members = (tree_owner.TreeMember("x", b"bytes", 0o644),)
    exact = 1 if limit == "MAX_TREE_FILES" else len(b"bytes")
    monkeypatch.setattr(tree_owner, limit, exact)
    owner = tree_owner.OriginalTree(tmp_path, members); owner.close()
    monkeypatch.setattr(tree_owner, limit, exact - 1)
    with pytest.raises(ArchiveError): tree_owner.OriginalTree(tmp_path, members)
