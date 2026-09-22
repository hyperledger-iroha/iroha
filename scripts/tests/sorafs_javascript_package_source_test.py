"""Exact captured source/member joins, never installed execution qualification."""
from __future__ import annotations

from dataclasses import FrozenInstanceError, replace
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import subprocess
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_package_source as package
from sorafs_javascript_package_fixtures import CHECKSUM, REQUIRED_OUTPUTS, archive_bytes, expected_files, source_inputs
from sorafs_javascript_dependencies import parse_dependency_lock


@pytest.fixture(scope="module")
def captured():
    sources, lock = source_inputs()
    return sources, lock, archive_bytes(expected_files(sources))


def project(sources, lock, checksum=CHECKSUM):
    return package.package_projection(sources, checksum_manifest=checksum, lock=lock)


def verify(raw, sources, lock, checksum=CHECKSUM):
    return package.verify_package_content(raw, sources=sources, checksum_manifest=checksum, lock=lock)


def test_actual_candidate_sources_have_the_complete_reviewed_projection(captured):
    sources, lock, _ = captured
    projected = project(sources, lock)
    expected = expected_files(sources)
    assert len(sources) == 202 and len(projected.members) == 199
    assert projected.version == "0.0.3"
    assert {row.name: row.content for row in projected.members} == expected
    assert sum(row.name.startswith("dist/") for row in projected.members) == 160
    assert projected.source_sha256 == tuple((name, hashlib.sha256(body).hexdigest()) for name, body in sorted(sources.items()))
    for row in projected.members:
        if row.name == package.CHECKSUM_MEMBER:
            assert row.source == "@original-native-checksum" and row.content is CHECKSUM
        else:
            expected_source = "src/" + row.name.removeprefix("dist/") if row.name.startswith("dist/") else row.name
            assert row.source == expected_source and row.content is sources[expected_source]
    assert not any(row.name.endswith(".node") or row.name.startswith(("src/", "scripts/")) for row in projected.members)
    with pytest.raises(FrozenInstanceError): projected.version = "forged"
    with pytest.raises(FrozenInstanceError): projected.members[0].source = "forged"


def test_original_tar_bytes_and_every_member_join_to_frozen_source(captured):
    sources, lock, raw = captured
    observed = verify(raw, sources, lock)
    assert observed.archive.raw is raw
    assert observed.archive.files() == expected_files(sources)
    assert all(row.mode == 0o644 for row in observed.archive.members)
    with pytest.raises(FrozenInstanceError): observed.archive = None
    reordered = verify(archive_bytes(expected_files(sources), reverse=True), sources, lock)
    assert reordered.projection == observed.projection


@pytest.mark.parametrize("name", ("dist/index.js", "dist/native.js", "README.md", "package.json",
                                   "recipes/README.md", "native/iroha_js_host.checksums.json"))
def test_resealed_member_substitution_rejects_even_with_valid_gzip_tar(captured, name):
    sources, lock, _ = captured
    files = expected_files(sources); files[name] += b"\nsubstituted bytes"
    with pytest.raises(package.ArchiveError, match="exact source/member/permission"):
        verify(archive_bytes(files), sources, lock)


@pytest.mark.parametrize("name", ("src/index.js", "src/native.js", "src/validationError.js", "README.md", "recipes/README.md"))
def test_changed_source_cannot_relabel_an_existing_package(captured, name):
    sources, lock, raw = captured
    changed = dict(sources); changed[name] += b"\nchanged candidate source"
    with pytest.raises(package.ArchiveError, match="exact source/member/permission"):
        verify(raw, changed, lock)


@pytest.mark.parametrize("name", ("dist/index.js", "dist/native.js", "recipes/README.md", "package.json", package.CHECKSUM_MEMBER))
def test_missing_exact_package_member_rejects(captured, name):
    sources, lock, _ = captured
    files = expected_files(sources); files.pop(name)
    with pytest.raises(package.ArchiveError): verify(archive_bytes(files), sources, lock)


@pytest.mark.parametrize("name", ("dist/foreign.js", "src/index.js", "scripts/build-dist.mjs",
                                   "native/iroha_js_host.node", "node_modules/foreign/index.js", "recipes/foreign.mjs"))
def test_extra_package_member_rejects(captured, name):
    sources, lock, _ = captured
    files = expected_files(sources); files[name] = b"inert unowned member"
    with pytest.raises(package.ArchiveError): verify(archive_bytes(files), sources, lock)


@pytest.mark.parametrize("mode", (0o755, 0o600, 0o644 | 0o4000))
def test_package_permission_relation_is_exact(captured, mode):
    sources, lock, _ = captured
    with pytest.raises(package.ArchiveError):
        verify(archive_bytes(expected_files(sources), modes={package.CHECKSUM_MEMBER: mode}), sources, lock)


@pytest.mark.parametrize("extra", (("README.md", b"duplicate"), ("readme.md", b"case alias"),
                                    ("dist", b"file replacing ancestor")))
def test_duplicate_alias_or_ancestor_tar_cannot_hide_in_equal_file_map(captured, extra):
    sources, lock, _ = captured
    with pytest.raises(package.ArchiveError):
        verify(archive_bytes(expected_files(sources), extra_rows=[extra]), sources, lock)


@pytest.mark.parametrize("name", ("package.json", "scripts/build-dist.mjs", "scripts/check-node-engine.mjs",
                                   "scripts/node-engine-contract.mjs", "package-lock.json", "index.d.ts",
                                   "recipes/README.md", "src/index.js", "src/toriiTestHooks.js", "src/public/sorafs.js"))
def test_missing_fixed_source_input_rejects(captured, name):
    sources, lock, _ = captured
    changed = dict(sources); changed.pop(name)
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("relative", REQUIRED_OUTPUTS)
def test_every_pinned_build_required_output_is_required_in_source(captured, relative):
    sources, lock, _ = captured
    changed = dict(sources); changed.pop("src/" + relative)
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("name", ("dist/foreign.js", package.CHECKSUM_MEMBER, "scripts/foreign.mjs", "recipes/foreign.mjs"))
def test_source_census_cannot_select_unowned_extra_input(captured, name):
    sources, lock, _ = captured
    changed = dict(sources); changed[name] = b"unowned"
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("name", (".npmignore", ".gitignore", ".npmrc", "src/.npmignore", "src/.GITIGNORE",
                                   "src/.NPMRC/a.js", "src/node_modules/a.js", "src/NODE_MODULES/a.js", "npm-shrinkwrap.json"))
def test_captured_selection_inputs_cannot_be_silently_ignored(captured, name):
    sources, lock, _ = captured
    changed = dict(sources); changed[name] = b"foreign selector"
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("names", (("src/Case.js", "src/case.js"), ("src/a.js", "src/a.js/b.js"),
                                    ("src/A/b.js", "src/a/c.js")))
def test_source_projection_reserves_same_alias_and_ancestor_namespace(captured, names):
    sources, lock, _ = captured
    changed = dict(sources); changed.update({name: b"inert" for name in names})
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("name", ("src/../a.js", "src//a.js", "src/a\\b.js", "src/e\u0301.js",
                                   "src/\ud800.js", "src/a*.js"))
def test_noncanonical_source_names_fail_with_owned_error(captured, name):
    sources, lock, _ = captured
    changed = dict(sources); changed[name] = b"inert"
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("body", (None, bytearray(b"inert"), memoryview(b"inert"), "inert", 1))
def test_source_values_are_exact_immutable_bytes(captured, body):
    sources, lock, _ = captured
    changed = dict(sources); changed["src/index.js"] = body
    with pytest.raises(package.ArchiveError): project(changed, lock)


@pytest.mark.parametrize("checksum", (b"", b"[]", b"null", b"NaN", b"\xff", b'{"x":1,"x":2}', bytearray(b"{}")))
def test_checksum_is_one_separate_bounded_duplicate_free_original_object(captured, checksum):
    sources, lock, _ = captured
    with pytest.raises(package.ArchiveError): project(sources, lock, checksum)


def test_checksum_content_is_opaque_without_native_qualification_claim(captured):
    sources, lock, raw = captured
    changed = b'{"different_original":"semantic verification belongs to native owner"}'
    with pytest.raises(package.ArchiveError): verify(raw, sources, lock, changed)
    accepted = verify(archive_bytes(expected_files(sources, changed)), sources, lock, changed)
    assert next(row for row in accepted.projection.members if row.name == package.CHECKSUM_MEMBER).content is changed


@pytest.mark.parametrize("name", ("package.json", "scripts/build-dist.mjs", "scripts/check-node-engine.mjs",
                                   "scripts/node-engine-contract.mjs"))
def test_recipe_bytes_cannot_be_reformatted_replaced_or_reselected(captured, name):
    sources, lock, _ = captured
    changed = dict(sources); changed[name] += b"\n"
    with pytest.raises(package.ArchiveError, match="recipe differs"):
        project(changed, lock)


def test_lock_projection_and_original_source_lock_must_rederive(captured):
    sources, lock, _ = captured
    for forged in (replace(lock, sdk_version="0.0.4"), replace(lock, dependencies=tuple(reversed(lock.dependencies))), object()):
        with pytest.raises(package.ArchiveError, match="source lock"):
            project(sources, forged)
    changed = dict(sources); changed["package-lock.json"] += b"\n"
    with pytest.raises(package.ArchiveError, match="source lock"):
        project(changed, lock)


def test_resealed_valid_lock_cannot_change_pinned_sdk_version(captured):
    sources, _, _ = captured
    value = json.loads(sources["package-lock.json"])
    value["version"] = value["packages"][""]["version"] = "0.0.4"
    raw = json.dumps(value).encode()
    lock = parse_dependency_lock(raw, expected_sha256=hashlib.sha256(raw).hexdigest())
    changed = dict(sources); changed["package-lock.json"] = raw
    with pytest.raises(package.ArchiveError, match="identity differs"):
        project(changed, lock)


def test_projection_and_original_content_perform_no_io_or_execution(captured, monkeypatch):
    sources, lock, raw = captured
    def forbidden(*args, **kwargs):
        raise AssertionError("pure package relation performed I/O or execution")
    with monkeypatch.context() as patch:
        patch.setattr("builtins.open", forbidden)
        for owner, names in ((io, ("open",)), (os, ("open", "stat", "lstat", "listdir", "scandir")),
                             (Path, ("open", "read_bytes", "resolve")),
                             (subprocess, ("run", "Popen")), (socket, ("create_connection",))):
            for name in names: patch.setattr(owner, name, forbidden)
        observed = verify(raw, sources, lock)
    assert len(observed.projection.members) == 199


@pytest.mark.parametrize("name", ("src/._hidden.js", "src/CVS/a.js", "src/.git/a.js", "src/a.orig/b.js",
                                   "src/foreign/a.js", "src/public/nested/a.js", "src/a.json", "src/program.wasm"))
def test_source_recipe_refuses_npm_ignored_or_unreviewed_layouts(captured, name):
    sources, lock, _ = captured
    changed = dict(sources); changed[name] = b"inert"
    with pytest.raises(package.ArchiveError, match="unreviewed module path"):
        project(changed, lock)


def test_projection_retains_bytes_after_callers_mutate_input_dictionary(captured):
    sources, lock, _ = captured
    changed = dict(sources)
    projection = project(changed, lock)
    row = next(row for row in projection.members if row.name == "dist/index.js")
    original = row.content
    changed["src/index.js"] = b"replacement"
    changed.clear()
    assert row.content is original and row.content == sources["src/index.js"]
    assert dict(projection.source_sha256)["src/index.js"] == hashlib.sha256(original).hexdigest()
