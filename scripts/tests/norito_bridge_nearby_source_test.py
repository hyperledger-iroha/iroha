"""Admit only the reviewed public Nearby source originals through strict custody."""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
from unittest import mock

import pytest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("nearby_source_seal", ROOT / "scripts/norito_bridge_source_seal.py")
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(seal)
REVIEWED = tuple(seal._REVIEWED_PUBLIC_NEARBY_SOURCE_PINS)


def test_reviewed_nearby_roles_are_exact_and_do_not_enable_generic_suffixes() -> None:
    assert len(REVIEWED) == 90
    assert sum(Path(path).suffix in {".m", ".mm"} for path in REVIEWED) == 86
    assert "vendor/nearby/.gitmodules" in REVIEWED
    assert seal._REVIEWED_PUBLIC_NEARBY_OUTPUT_INPUTS == frozenset({
        "vendor/nearby/third_party/json/json/include/nlohmann/detail/output/binary_writer.hpp",
        "vendor/nearby/third_party/json/json/include/nlohmann/detail/output/output_adapters.hpp",
        "vendor/nearby/third_party/json/json/include/nlohmann/detail/output/serializer.hpp",
    })
    assert set(REVIEWED) == {
        path for path in REVIEWED if Path(path).suffix in {".m", ".mm"}
    } | {"vendor/nearby/.gitmodules"} | seal._REVIEWED_PUBLIC_NEARBY_OUTPUT_INPUTS
    assert not {".m", ".mm"} & seal._PUBLIC_SOURCE_SUFFIXES
    assert "output" in seal._PROHIBITED_SOURCE_PARTS


@pytest.mark.parametrize("relative", REVIEWED)
def test_each_reviewed_nearby_original_matches_independent_pins_and_upstream_blob(relative: str) -> None:
    original = (ROOT / relative).read_bytes()
    expected_hash, expected_size = seal._REVIEWED_PUBLIC_NEARBY_SOURCE_PINS[relative]
    assert len(original) == expected_size
    assert hashlib.sha256(original).hexdigest() == expected_hash
    upstream = json.loads((ROOT / "vendor/nearby/UPSTREAM.json").read_text())
    record = upstream["files"][relative.removeprefix("vendor/nearby/")]
    assert record["sha256"] == expected_hash
    git_blob = b"blob " + str(len(original)).encode() + b"\0" + original
    assert record["git_blob"] == hashlib.sha1(git_blob).hexdigest()
    assert seal._read_public_source_bytes(ROOT, relative) == original


@pytest.mark.parametrize("relative", REVIEWED)
def test_changed_reviewed_nearby_source_is_refused(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    path = root / relative
    path.parent.mkdir(parents=True)
    original = (ROOT / relative).read_bytes()
    path.write_bytes(original)
    assert seal._read_public_source_bytes(root, relative) == original
    # Same-length substitution defeats a size-only check while preserving syntax
    # where the leading byte happens to be a comment delimiter.
    path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
    with pytest.raises(RuntimeError, match="reviewed Nearby source differs"):
        seal._read_public_source_bytes(root, relative)


@pytest.mark.parametrize("relative", REVIEWED)
def test_reviewed_nearby_leaf_alias_is_refused_before_open(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    path = root / relative
    path.parent.mkdir(parents=True)
    path.symlink_to(ROOT / relative)
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source descriptor permitted")) as opened:
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()


@pytest.mark.parametrize("relative", [
    "vendor/nearby/.gitmodules",
    "vendor/nearby/connections/swift/NearbyCoreAdapter/Sources/CPPInputStream.mm",
    "vendor/nearby/internal/platform/implementation/apple/GNCDevicePaths.m",
    "vendor/nearby/third_party/json/json/include/nlohmann/detail/output/serializer.hpp",
])
def test_reviewed_nearby_ancestor_alias_is_refused_before_open(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    (root / "vendor").mkdir()
    (root / "vendor/nearby").symlink_to(ROOT / "vendor/nearby", target_is_directory=True)
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source descriptor permitted")) as opened:
        with pytest.raises(RuntimeError, match="symlinked"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()


@pytest.mark.parametrize("relative", [
    "vendor/other/.gitmodules",
    "vendor/nearby/Other.mm",
    "vendor/other/GNCDevicePaths.m",
    "vendor/nearby/internal/platform/implementation/apple/GNCDevicePaths.m.backup",
    "vendor/nearby/third_party/json/json/include/nlohmann/detail/output/other.hpp",
    "vendor/other/third_party/json/json/include/nlohmann/detail/output/serializer.hpp",
    "vendor/nearby/credentials/serializer.hpp",
    "vendor/nearby/output/serializer.hpp",
    "vendor/nearby/private_key.mm",
])
def test_unreviewed_neighbor_is_refused_before_open(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    path = root / relative
    path.parent.mkdir(parents=True)
    path.write_bytes(b"public refusal sentinel\n")
    with mock.patch.object(seal.os, "open", side_effect=AssertionError("no source descriptor permitted")) as opened:
        with pytest.raises(RuntimeError, match="prohibited|admitted public filename"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()


@pytest.mark.parametrize("relative", [
    "vendor/nearby/credentials/serializer.hpp",
    "vendor/nearby/private/serializer.hpp",
    "vendor/nearby/auth_token.mm",
    "vendor/nearby/vultr-public.mm",
    "vendor/nearby/public.pem",
])
def test_even_expanded_pins_do_not_override_material_or_provider_refusal(tmp_path: Path, relative: str) -> None:
    root = tmp_path.resolve()
    path = root / relative
    path.parent.mkdir(parents=True)
    path.write_bytes(b"public refusal sentinel\n")
    with mock.patch.dict(seal._REVIEWED_PUBLIC_NEARBY_SOURCE_PINS, {relative: ("0" * 64, 0)}), \
            mock.patch.object(seal.os, "open", side_effect=AssertionError("no source descriptor permitted")) as opened:
        with pytest.raises(RuntimeError, match="prohibited"):
            seal._read_public_source_bytes(root, relative)
    opened.assert_not_called()
