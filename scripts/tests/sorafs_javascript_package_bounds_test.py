"""Admission capacity and sole namespace controls with reduced exact limits."""
from __future__ import annotations

from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sorafs_javascript_archive as archive
import sorafs_javascript_package_source as package
from sorafs_javascript_package_fixtures import CHECKSUM, expected_files, source_inputs


@pytest.fixture(scope="module")
def captured():
    return source_inputs()


def project(sources, lock):
    return package.package_projection(sources, checksum_manifest=CHECKSUM, lock=lock)


def test_fixed_source_capacity_profile():
    assert package.MAX_SOURCE_FILES == 4096
    assert package.MAX_SOURCE_FILE_BYTES == 16 * 1024 * 1024
    assert package.MAX_SOURCE_BYTES == 64 * 1024 * 1024
    assert package.MAX_CHECKSUM_BYTES == 1024 * 1024


@pytest.mark.parametrize("limit,observed", (
    ("MAX_SOURCE_FILES", lambda sources: len(sources)),
    ("MAX_SOURCE_FILE_BYTES", lambda sources: max(map(len, sources.values()))),
    ("MAX_SOURCE_BYTES", lambda sources: sum(map(len, sources.values()))),
))
def test_source_capacity_exact_then_one_under_before_content_parse(captured, monkeypatch, limit, observed):
    sources, lock = captured
    maximum = observed(sources)
    monkeypatch.setattr(package, limit, maximum)
    assert len(project(sources, lock).members) == 199
    monkeypatch.setattr(package, limit, maximum - 1)
    def forbidden(*args, **kwargs):
        raise AssertionError("source input not admitted before metadata parsing")
    monkeypatch.setattr(package, "parse_dependency_lock", forbidden)
    with pytest.raises(package.ArchiveError, match="source inventory or byte bound"):
        project(sources, lock)


def test_checksum_capacity_exact_then_one_under(captured, monkeypatch):
    sources, lock = captured
    monkeypatch.setattr(package, "MAX_CHECKSUM_BYTES", len(CHECKSUM))
    assert len(project(sources, lock).members) == 199
    monkeypatch.setattr(package, "MAX_CHECKSUM_BYTES", len(CHECKSUM) - 1)
    with pytest.raises(package.ArchiveError, match="JSON byte bound"):
        project(sources, lock)


@pytest.mark.parametrize("limit", ("MAX_MEMBERS", "MAX_PATH_NODES", "MAX_PATH_BYTES"))
def test_projection_uses_archive_owner_exact_namespace_capacity(captured, monkeypatch, limit):
    sources, lock = captured
    names = set(expected_files(sources))
    nodes = {"/".join(name.split("/")[:length]) for name in names for length in range(1, len(name.split("/")) + 1)}
    value = {"MAX_MEMBERS": len(names), "MAX_PATH_NODES": len(nodes),
             "MAX_PATH_BYTES": sum(len(name.encode()) for name in nodes)}[limit]
    monkeypatch.setattr(archive, limit, value)
    assert len(project(sources, lock).members) == 199
    monkeypatch.setattr(archive, limit, value - 1)
    with pytest.raises(package.ArchiveError, match="count bound|ownership count or byte bound"):
        project(sources, lock)


def test_invalid_recipe_is_rejected_before_original_tar_inflation(captured, monkeypatch):
    sources, lock = captured
    changed = dict(sources); changed["scripts/build-dist.mjs"] += b"\n"
    def forbidden(*args, **kwargs):
        raise AssertionError("invalid source reached tar inflation")
    monkeypatch.setattr(package, "parse_npm_archive", forbidden)
    with pytest.raises(package.ArchiveError, match="recipe differs"):
        package.verify_package_content(b"not a tar", sources=changed, checksum_manifest=CHECKSUM, lock=lock)


@pytest.mark.parametrize("sources", (None, [], (), b"{}", {}, {"src/index.js": bytearray(b"x")}))
def test_malformed_inventory_is_an_owned_error(captured, sources):
    _, lock = captured
    with pytest.raises(package.ArchiveError): project(sources, lock)


@pytest.mark.parametrize("name", ("a/../b", "\ud800.js", "", "a\\b", "A:thing", 3))
def test_shared_path_owner_canonical_refusal_poisons_instance(name):
    paths = archive.NpmPathInventory()
    with pytest.raises(archive.ArchiveError): paths.admit(name)
    with pytest.raises(archive.ArchiveError): paths.admit("otherwise-valid.js")


def test_shared_namespace_cannot_continue_after_duplicate_failure():
    paths = archive.NpmPathInventory()
    paths.admit("a.js")
    with pytest.raises(archive.ArchiveError): paths.admit("A.js")
    with pytest.raises(archive.ArchiveError): paths.admit("unrelated.js")


def test_shared_namespace_cannot_continue_after_partial_capacity_failure(monkeypatch):
    monkeypatch.setattr(archive, "MAX_PATH_NODES", 1)
    paths = archive.NpmPathInventory()
    with pytest.raises(archive.ArchiveError): paths.admit("dir/file.js")
    monkeypatch.setattr(archive, "MAX_PATH_NODES", 100)
    with pytest.raises(archive.ArchiveError): paths.admit("dir/other.js")


def test_overlong_source_name_is_rejected_before_utf8_encoding(captured):
    sources, lock = captured
    changed = dict(sources)
    # A surrogate would fail UTF-8 encoding; character admission must run first.
    changed["src/" + "a" * archive.MAX_NAME_BYTES + "\ud800.js"] = b"inert"
    with pytest.raises(package.ArchiveError, match="canonical spelling"):
        project(changed, lock)
    with pytest.raises(archive.ArchiveError, match="character bound"):
        archive._path("package/" + "a" * archive.MAX_NAME_BYTES + "\ud800")
