"""Exact original dependency placement controls; no installed execution claims."""
from __future__ import annotations

import base64
from dataclasses import replace
import gzip
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_javascript_dependencies as dependencies


def _json(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def _lock_value():
    return json.loads((ROOT / "javascript/iroha_js/package-lock.json").read_bytes())


def _parse(value):
    raw = _json(value)
    return dependencies.parse_dependency_lock(raw, expected_sha256=hashlib.sha256(raw).hexdigest())


def _archive(files):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name, raw in sorted(files.items()):
            info = tarfile.TarInfo("package/" + name)
            info.mode, info.size = 0o644, len(raw)
            archive.addfile(info, io.BytesIO(raw))
    return gzip.compress(output.getvalue(), mtime=0)


def _originals(*, alter=None):
    value, originals = _lock_value(), {}
    for location, version in dependencies.VERSIONS.items():
        row = value["packages"][location]
        package = {"name": location.rsplit("node_modules/", 1)[-1], "version": version,
                   "dependencies": row.get("dependencies", {}), "engines": row.get("engines", {}),
                   "scripts": {"test": "this inert fixture is never executed"}}
        files = {"package.json": _json(package), "index.js": location.encode()}
        if alter is not None and location == "node_modules/@scure/bip39":
            alter(package, files)
        raw = _archive(files)
        row["integrity"] = "sha512-" + base64.b64encode(hashlib.sha512(raw).digest()).decode()
        originals[location] = raw
    return _parse(value), originals


def test_actual_candidate_lock_preserves_both_hashes_locations_and_exact_versions():
    raw = (ROOT / "javascript/iroha_js/package-lock.json").read_bytes()
    parsed = dependencies.parse_dependency_lock(raw, expected_sha256=hashlib.sha256(raw).hexdigest())
    assert parsed.raw is raw
    assert parsed.sdk_version == _lock_value()["version"]
    assert [(row.location, row.version) for row in parsed.dependencies] == list(dependencies.VERSIONS.items())
    hashes = [row for row in parsed.dependencies if row.name == "@noble/hashes"]
    assert [row.version for row in hashes] == ["1.8.0", "2.2.0"]
    assert hashes[1].location == "node_modules/@scure/bip39/node_modules/@noble/hashes"
    assert hashes[1].engines == (("node", ">= 20.19.0"),)


@pytest.mark.parametrize("pin", (None, False, "0" * 64, "G" * 64, "a" * 63, "sha256:" + "a" * 64))
def test_independent_lock_pin_is_required(pin):
    with pytest.raises(dependencies.ArchiveError, match="independently selected"):
        dependencies.parse_dependency_lock(_json(_lock_value()), expected_sha256=pin)


@pytest.mark.parametrize("mutation", (
    lambda row: row.update(lockfileVersion=True), lambda row: row.update(lockfileVersion=3.0),
    lambda row: row.update(lockfileVersion=2), lambda row: row.update(requires=1),
    lambda row: row.update(name="@another/package"), lambda row: row.update(version="00.0.3"),
    lambda row: row.update(version="0.0.3-beta"), lambda row: row.update(unknown=True),
    lambda row: row.update(packages=[]), lambda row: row["packages"].pop(""),
    lambda row: row["packages"][""].update(version="0.0.4"),
    lambda row: row["packages"][""].update(dependencies={}),
    lambda row: row["packages"][""].update(workspaces=["other"]),
    lambda row: row["packages"].update({"node_modules/unowned": {"version": "1.0.0"}}),
    lambda row: row["packages"].pop("node_modules/@scure/bip39/node_modules/@noble/hashes"),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(dev=1),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(dev=True),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(optional=True),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(hasInstallScript=True),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(version="2.2.0"),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(resolved="file:local.tgz"),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(resolved="https://foreign.example/hashes.tgz"),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(integrity="sha256-" + "A" * 88),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(integrity="sha512-" + "A" * 85 + "B=="),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(integrity="sha512-" + "A" * 86 + "== sha512-x"),
    lambda row: row["packages"]["node_modules/@noble/curves"].update(dependencies={}),
    lambda row: row["packages"]["node_modules/@scure/bip39"].update(dependencies={"@noble/hashes": "1.8.0", "@scure/base": "2.2.0"}),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(engines={"npm": "*"}),
    lambda row: row["packages"]["node_modules/@noble/hashes"].update(engines={"node": True}),
))
def test_resealed_lock_cannot_change_fixed_graph_or_install_policy(mutation):
    row = _lock_value()
    mutation(row)
    with pytest.raises(dependencies.ArchiveError):
        _parse(row)


@pytest.mark.parametrize("raw", (b'{"name":"one","name":"two"}', b'{"x":NaN}', b'[]',
                                   b'\xff', b'{"x":' + b'[' * 2000 + b'0' + b']' * 2000 + b'}'))
def test_original_json_duplicate_nonfinite_utf8_shape_and_recursion_reject(raw):
    with pytest.raises(dependencies.ArchiveError):
        dependencies.parse_dependency_lock(raw, expected_sha256=hashlib.sha256(raw).hexdigest())


def test_original_archive_content_and_location_are_joined_without_io(monkeypatch):
    lock, originals = _originals()
    def forbidden(*args, **kwargs):
        raise AssertionError("content-only verifier attempted I/O or execution")
    with monkeypatch.context() as patch:
        patch.setattr("builtins.open", forbidden)
        patch.setattr(Path, "open", forbidden)
        patch.setattr(subprocess, "run", forbidden)
        patch.setattr(subprocess, "Popen", forbidden)
        parsed = dependencies.parse_dependency_archives(lock, originals)
    assert len(parsed) == 9
    assert sum(row.archive.tar_size for row in parsed) == 9 * 10240
    for row in parsed:
        assert row.archive.raw is originals[row.dependency.location]
        assert row.archive.files()["index.js"] == row.dependency.location.encode()


@pytest.mark.parametrize("mutation", ("missing", "extra", "renamed", "wrong_type", "empty", "swapped", "changed"))
def test_original_inventory_integrity_and_nested_ownership_cannot_be_substituted(mutation):
    lock, originals = _originals()
    location = "node_modules/@noble/hashes"
    if mutation == "missing": originals.pop(location)
    if mutation == "extra": originals["node_modules/unowned"] = b"x"
    if mutation == "renamed": originals["@noble/hashes"] = originals.pop(location)
    if mutation == "wrong_type": originals[location] = bytearray(originals[location])
    if mutation == "empty": originals[location] = b""
    if mutation == "swapped": originals[location] = originals["node_modules/@scure/bip39/node_modules/@noble/hashes"]
    if mutation == "changed": originals[location] += b"\0"
    with pytest.raises(dependencies.ArchiveError):
        dependencies.parse_dependency_archives(lock, originals)


def test_correct_digests_cannot_relabel_root_hashes_as_nested_version():
    lock, originals = _originals()
    nested = "node_modules/@scure/bip39/node_modules/@noble/hashes"
    originals[nested] = originals["node_modules/@noble/hashes"]
    value = json.loads(lock.raw)
    value["packages"][nested]["integrity"] = value["packages"]["node_modules/@noble/hashes"]["integrity"]
    with pytest.raises(dependencies.ArchiveError, match="metadata differs"):
        dependencies.parse_dependency_archives(_parse(value), originals)


@pytest.mark.parametrize("projection", ("sdk", "order", "location", "integrity", "engines"))
def test_immutable_projection_must_rederive_original_lock(projection):
    lock, originals = _originals()
    rows = list(lock.dependencies)
    if projection == "sdk": lock = replace(lock, sdk_version="99.0.0")
    elif projection == "order": lock = replace(lock, dependencies=tuple(reversed(rows)))
    else:
        field, value = {"location": ("location", "foreign"), "integrity": ("integrity", "foreign"),
                        "engines": ("engines", (("node", "*"),))}[projection]
        rows[0] = replace(rows[0], **{field: value})
        lock = replace(lock, dependencies=tuple(rows))
    with pytest.raises(dependencies.ArchiveError, match="projection"):
        dependencies.parse_dependency_archives(lock, originals)


@pytest.mark.parametrize("change", (
    {"name": "foreign"}, {"version": "1.8.0"}, {"dependencies": {}},
    {"dependencies": {"@noble/hashes": "1.8.0", "@scure/base": "2.2.0"}},
    {"engines": {"node": "*"}}, {"scripts": []},
    *({key: {}} for key in ("optionalDependencies", "peerDependencies", "peerDependenciesMeta",
                           "bundleDependencies", "bundledDependencies", "workspaces", "bin", "gypfile", "man")),
    {"directories": {"bin": "review-bin"}}, {"acceptDependencies": {"@noble/hashes": "*"}},
    {"overrides": {"@noble/hashes": "1.8.0"}}, {"unknown_future_resolution": True},
    {"os": ["darwin"]}, {"cpu": ["arm64"]}, {"libc": ["glibc"]},
    *({"scripts": {key: "exit 0"}} for key in ("preinstall", "install", "postinstall", "prepare", "prepublish")),
))
def test_resealed_archive_metadata_cannot_change_resolution_or_install_handlers(change):
    def alter(package, files):
        files["package.json"] = _json(package | change)
    lock, originals = _originals(alter=alter)
    with pytest.raises(dependencies.ArchiveError):
        dependencies.parse_dependency_archives(lock, originals)


@pytest.mark.parametrize("name", ("node_modules/@noble/hashes/index.js", "lib/NODE_MODULES/foreign.js",
                                     ".npmrc", "lib/.NPMRC", "npm-shrinkwrap.json", "package-lock.json",
                                     "native.node", "program.wasm", "program.wasi",
                                     "addon.so", "addon.dll", "addon.dylib", "binding.gyp"))
def test_pinned_packages_cannot_preseed_other_dependency_or_startup_owners(name):
    lock, originals = _originals(alter=lambda package, files: files.update({name: b"unowned"}))
    with pytest.raises(dependencies.ArchiveError, match="bundled, startup, native or retired"):
        dependencies.parse_dependency_archives(lock, originals)


@pytest.mark.parametrize("body", (None, b'{}', b'{"name":"x","name":"y"}', b'[]', b'\xff'))
def test_original_package_json_is_required_bounded_and_duplicate_free(body):
    def alter(package, files):
        if body is None: files.pop("package.json")
        else: files["package.json"] = body
    lock, originals = _originals(alter=alter)
    with pytest.raises(dependencies.ArchiveError):
        dependencies.parse_dependency_archives(lock, originals)


def test_aggregate_compressed_and_inflated_capacity_exact_then_one_under(monkeypatch):
    lock, originals = _originals()
    compressed = sum(map(len, originals.values()))
    monkeypatch.setattr(dependencies, "MAX_ARCHIVES_BYTES", compressed)
    monkeypatch.setattr(dependencies, "MAX_TARS_BYTES", 9 * 10240)
    assert len(dependencies.parse_dependency_archives(lock, originals)) == 9
    monkeypatch.setattr(dependencies, "MAX_TARS_BYTES", 9 * 10240 - 1)
    with pytest.raises(dependencies.ArchiveError, match="tar byte bound"):
        dependencies.parse_dependency_archives(lock, originals)
    monkeypatch.setattr(dependencies, "MAX_ARCHIVES_BYTES", compressed - 1)
    def forbidden(*args, **kwargs):
        raise AssertionError("must reserve compressed inventory before inflation")
    monkeypatch.setattr(dependencies, "parse_npm_archive", forbidden)
    with pytest.raises(dependencies.ArchiveError, match="aggregate compressed"):
        dependencies.parse_dependency_archives(lock, originals)


def test_original_integrity_is_checked_before_decompression(monkeypatch):
    lock, originals = _originals()
    originals[next(iter(originals))] += b"changed"
    def forbidden(*args, **kwargs):
        raise AssertionError("unowned original reached decompression")
    monkeypatch.setattr(dependencies, "parse_npm_archive", forbidden)
    with pytest.raises(dependencies.ArchiveError, match="lock integrity"):
        dependencies.parse_dependency_archives(lock, originals)


def test_metadata_admission_exact_then_over(monkeypatch):
    value = _lock_value()
    raw = _json(value)
    monkeypatch.setattr(dependencies, "MAX_LOCK_BYTES", len(raw))
    assert _parse(value).raw == raw
    monkeypatch.setattr(dependencies, "MAX_LOCK_BYTES", len(raw) - 1)
    with pytest.raises(dependencies.ArchiveError, match="lock byte bound"):
        _parse(value)
    monkeypatch.undo()
    lock, originals = _originals()
    largest = max(len(row.archive.files()["package.json"])
                  for row in dependencies.parse_dependency_archives(lock, originals))
    monkeypatch.setattr(dependencies, "MAX_PACKAGE_JSON_BYTES", largest)
    assert len(dependencies.parse_dependency_archives(lock, originals)) == 9
    monkeypatch.setattr(dependencies, "MAX_PACKAGE_JSON_BYTES", largest - 1)
    with pytest.raises(dependencies.ArchiveError, match="JSON byte bound"):
        dependencies.parse_dependency_archives(lock, originals)
