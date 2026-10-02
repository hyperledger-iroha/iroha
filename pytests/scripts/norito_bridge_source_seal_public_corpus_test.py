"""Exact authenticated public parser corpus roles; no Cargo, fuzz execution or secret inputs."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("norito_reviewed_corpus", ROOT / "scripts/norito_bridge_source_seal.py")
assert SPEC and SPEC.loader
seal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(seal)
CATALOG = ROOT / seal._NORITO_PUBLIC_CORPUS_MANIFEST
ENTRIES = json.loads(CATALOG.read_bytes())["entries"]
FIRST = ENTRIES[0]
REFUSED_ACTUAL = next(e for e in ENTRIES if e["path"].endswith("bb1548dd900b509f4fbb273748be0113649fcd77"))
BY_ROLE = [next(e for e in ENTRIES if Path(e["path"]).parts[4] == role)
           for role in sorted(seal._NORITO_PUBLIC_CORPUS_TARGETS)]


def install(root: Path, entry=FIRST) -> bytes:
    declaration = root / seal._NORITO_PUBLIC_CORPUS_MANIFEST
    declaration.parent.mkdir(parents=True, exist_ok=True)
    declaration.write_bytes(CATALOG.read_bytes())
    source = root / entry["path"]
    source.parent.mkdir(parents=True, exist_ok=True)
    raw = (ROOT / entry["path"]).read_bytes()
    assert hashlib.sha1(raw).hexdigest() == source.name
    assert hashlib.sha256(raw).hexdigest() == entry["sha256"]
    source.write_bytes(raw)
    return raw


@pytest.mark.parametrize("entry", BY_ROLE + [REFUSED_ACTUAL])
def test_actual_reviewed_seed_role_preserves_every_original_byte_without_git_index(tmp_path, entry):
    raw = install(tmp_path, entry)
    with mock.patch.object(seal, "run", side_effect=AssertionError("no Git or compiler permitted")):
        assert seal._read_public_source_bytes(tmp_path.resolve(), entry["path"]) == raw


def test_exact_reviewed_declaration_contains_only_741_authenticated_lowercase_seed_paths():
    assert len(ENTRIES) == 741
    assert hashlib.sha256(CATALOG.read_bytes()).hexdigest() == seal._NORITO_PUBLIC_CORPUS_MANIFEST_SHA256
    assert len({e["path"] for e in ENTRIES}) == 741
    assert [e["path"] for e in ENTRIES] == sorted(e["path"] for e in ENTRIES)
    for entry in ENTRIES:
        assert seal._norito_public_corpus_path(entry["path"])


@pytest.mark.parametrize("relative", [
    "crates/norito/fuzz/corpus/json_parse_string/" + "0" * 40,
    "crates/norito/fuzz/corpus/undeclared_role/" + "0" * 40,
    "crates/other/fixtures/" + "0" * 40,
    "crates/norito/fuzz/corpus/json_parse_string/" + "A" * 40,
    "crates/norito/fuzz/corpus/json_parse_string/" + "0" * 39,
    "crates/norito/fuzz/corpus/json_parse_string/" + "0" * 40 + "/neighbor",
])
def test_undeclared_or_neighbor_hex_input_is_refused_without_opening_input(tmp_path, relative):
    install(tmp_path)
    source = tmp_path / relative
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_bytes(b"PUBLIC SYNTHETIC NEIGHBOR; NO SECRET\n")
    original_open = os.open
    input_opens = []
    def observe(path, flags, *args, **kwargs):
        if str(path) == source.name: input_opens.append(str(path))
        return original_open(path, flags, *args, **kwargs)
    with mock.patch.object(seal.os, "open", side_effect=observe):
        with pytest.raises(RuntimeError, match="admitted public filename"):
            seal._read_public_source_bytes(tmp_path.resolve(), relative)
    assert input_opens == []


def test_seed_changed_at_same_path_is_refused_not_silently_fingerprinted(tmp_path):
    install(tmp_path)
    (tmp_path / FIRST["path"]).write_bytes(b"CHANGED PUBLIC SYNTHETIC ORIGINAL")
    with pytest.raises(RuntimeError, match="original differs from its reviewed role"):
        seal._read_public_source_bytes(tmp_path.resolve(), FIRST["path"])


def test_missing_declaration_is_refused_not_dropped_as_a_deleted_source(tmp_path):
    source = tmp_path / FIRST["path"]
    source.parent.mkdir(parents=True)
    source.write_bytes(b"PUBLIC SENTINEL")
    lock = tmp_path / "Cargo.lock"; lock.write_bytes(b"public lock\n")
    with mock.patch.object(seal, "source_seal_tools", return_value=(None,) * 4), \
            mock.patch.object(seal, "source_seal_environment", return_value={}), \
            mock.patch.object(seal, "run", return_value=FIRST["path"].encode() + b"\0"):
        with pytest.raises(RuntimeError, match="required public corpus declaration is missing"):
            seal.listed_files(tmp_path.resolve(), [FIRST["path"]], lock)


def test_changed_declaration_cannot_authorize_an_unreviewed_seed(tmp_path):
    install(tmp_path)
    declaration = tmp_path / seal._NORITO_PUBLIC_CORPUS_MANIFEST
    declaration.write_bytes(CATALOG.read_bytes().replace(b'"bytes": ', b'"changed_bytes": ', 1))
    with pytest.raises(RuntimeError, match="differs from its reviewed original"):
        seal._read_public_source_bytes(tmp_path.resolve(), FIRST["path"])


@pytest.mark.parametrize("mutation", ["duplicate_key", "duplicate_path", "unknown_field", "wrong_role", "material", "unordered", "bool_size"])
def test_declaration_schema_refuses_malformed_even_if_a_test_changes_the_review_pin(tmp_path, mutation):
    install(tmp_path)
    value = json.loads(CATALOG.read_bytes())
    if mutation == "duplicate_key":
        raw = CATALOG.read_bytes().replace(b'"schema":', b'"schema":"duplicate", "schema":', 1)
    else:
        if mutation == "duplicate_path": value["entries"][1] = value["entries"][0]
        elif mutation == "unknown_field": value["entries"][0]["unreviewed"] = True
        elif mutation == "wrong_role": value["entries"][0]["path"] = "crates/norito/fuzz/corpus/unreviewed/" + "0" * 40
        elif mutation == "material": value["entries"][0]["path"] = "crates/norito/fuzz/private/" + "0" * 40
        elif mutation == "unordered": value["entries"][0], value["entries"][1] = value["entries"][1], value["entries"][0]
        elif mutation == "bool_size": value["entries"][0]["bytes"] = True
        raw = json.dumps(value).encode()
    (tmp_path / seal._NORITO_PUBLIC_CORPUS_MANIFEST).write_bytes(raw)
    with mock.patch.object(seal, "_NORITO_PUBLIC_CORPUS_MANIFEST_SHA256", hashlib.sha256(raw).hexdigest()):
        with pytest.raises(RuntimeError, match="duplicate|unreviewed|prohibited|ordered"):
            seal._read_public_source_bytes(tmp_path.resolve(), FIRST["path"])


@pytest.mark.parametrize("relative", [
    "crates/norito/fuzz/private/" + "0" * 40,
    "crates/norito/fuzz/corpus/vultr/" + "0" * 40,
    "crates/norito/fuzz/corpus/json_parse_string/credentials.json",
])
def test_prohibited_role_is_refused_before_catalog_or_input_open(tmp_path, relative):
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no content descriptor")) as opened:
        with pytest.raises(RuntimeError, match="prohibited"):
            seal._read_public_source_bytes(tmp_path.resolve(), relative)
    opened.assert_not_called()


@pytest.mark.parametrize("alias_kind", ["seed", "ancestor", "declaration"])
def test_reviewed_seed_and_declaration_aliases_are_refused_without_opening_seed(tmp_path, alias_kind):
    raw = install(tmp_path)
    source = tmp_path / FIRST["path"]
    public = tmp_path / "public-sentinel.rs"; public.write_bytes(raw)
    if alias_kind == "seed": source.unlink(); source.symlink_to(public)
    elif alias_kind == "declaration":
        declaration = tmp_path / seal._NORITO_PUBLIC_CORPUS_MANIFEST
        declaration.unlink(); declaration.symlink_to(CATALOG)
    else:
        ancestor = source.parent
        renamed = ancestor.with_name(ancestor.name + "-original")
        ancestor.rename(renamed); ancestor.symlink_to(renamed, target_is_directory=True)
    original_open = os.open; seed_opens = []
    def observe(path, flags, *args, **kwargs):
        if str(path) == source.name: seed_opens.append(str(path))
        return original_open(path, flags, *args, **kwargs)
    with mock.patch.object(seal.os, "open", side_effect=observe):
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(tmp_path.resolve(), FIRST["path"])
    assert seed_opens == []


def test_seed_descriptor_swap_cannot_open_a_new_original(tmp_path):
    install(tmp_path); source = tmp_path / FIRST["path"]
    replacement = tmp_path / "replacement-public.rs"; replacement.write_bytes(b"PUBLIC SYNTHETIC OTHER")
    original_open = os.open
    def swap(path, flags, *args, **kwargs):
        if str(path) == source.name:
            source.unlink(); source.symlink_to(replacement)
        return original_open(path, flags, *args, **kwargs)
    with mock.patch.object(seal.os, "open", side_effect=swap):
        with pytest.raises((OSError, RuntimeError)):
            seal._read_public_source_bytes(tmp_path.resolve(), FIRST["path"])


def test_declaration_swap_during_seed_read_cannot_change_classification(tmp_path):
    install(tmp_path); source = tmp_path / FIRST["path"]
    declaration = tmp_path / seal._NORITO_PUBLIC_CORPUS_MANIFEST
    original_open = os.open
    def swap(path, flags, *args, **kwargs):
        if str(path) == source.name:
            declaration.write_bytes(CATALOG.read_bytes() + b" ")
        return original_open(path, flags, *args, **kwargs)
    with mock.patch.object(seal.os, "open", side_effect=swap):
        with pytest.raises(RuntimeError, match="reviewed original|changed while authenticating"):
            seal._read_public_source_bytes(tmp_path.resolve(), FIRST["path"])


def test_explicit_seed_selection_keeps_catalog_and_original_fingerprint_domain(tmp_path):
    raw = install(tmp_path)
    lock = tmp_path / "Cargo.lock"; lock.write_bytes(b"public lock\n")
    inventory = FIRST["path"].encode() + b"\0"
    with mock.patch.object(seal, "source_seal_tools", return_value=(None,) * 4), \
            mock.patch.object(seal, "source_seal_environment", return_value={}), \
            mock.patch.object(seal, "run", return_value=inventory):
        actual_files = seal.listed_files(tmp_path.resolve(), [FIRST["path"]], lock)
        assert actual_files == sorted([FIRST["path"], seal._NORITO_PUBLIC_CORPUS_MANIFEST])
        expected = hashlib.sha256()
        for relative in actual_files:
            original = raw if relative == FIRST["path"] else CATALOG.read_bytes()
            expected.update(relative.encode() + b"\0" + original + b"\0")
        expected.update(b"\0selected-cargo-lock-sha256\0" + hashlib.sha256(lock.read_bytes()).digest())
        assert seal.fingerprint(tmp_path.resolve(), [FIRST["path"]], lock) == expected.hexdigest()
        # An ignored/untracked local neighbor absent from the filename inventory
        # is neither recursively included nor opened by role declaration lookup.
        (tmp_path / FIRST["path"]).with_name("0" * 40).write_bytes(b"PUBLIC LOCAL NEIGHBOR")
        assert seal.listed_files(tmp_path.resolve(), [FIRST["path"]], lock) == actual_files


def test_empty_reviewed_role_cannot_admit_exact_seed_before_any_open(tmp_path):
    raw = install(tmp_path, REFUSED_ACTUAL)
    # An empty reviewed role cannot confer filename admission. This is a pure
    # shipping regression, independent of old scripts or target evidence paths.
    with mock.patch.object(seal, "_reviewed_norito_public_corpus", return_value=({}, {})), \
            mock.patch.object(seal.os, "open", side_effect=AssertionError("no seed descriptor")) as opened:
        with pytest.raises(RuntimeError, match="not an admitted public filename"):
            seal._read_public_source_bytes(tmp_path.resolve(), REFUSED_ACTUAL["path"])
    opened.assert_not_called()
    assert seal._read_public_source_bytes(tmp_path.resolve(), REFUSED_ACTUAL["path"]) == raw
