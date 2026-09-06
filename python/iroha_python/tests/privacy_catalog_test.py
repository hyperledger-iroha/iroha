"""Canonical first-release privacy catalog surface tests."""

from __future__ import annotations

import hashlib
import importlib
from pathlib import Path

import pytest

import iroha_python
from iroha_python.privacy_catalog import PRIVACY_PROTOCOL_IDS_V1

MATRIX_PATH = (
    Path(__file__).resolve().parents[3] / "fixtures" / "privacy" / "exact12_v1.tsv"
)
MATRIX_TEXT = MATRIX_PATH.read_text(encoding="utf-8")
MATRIX_ROWS = tuple(
    tuple(line.split("\t"))
    for line in MATRIX_TEXT.splitlines()
    if line and not line.startswith("#")
)


def _matrix_rows(kind: str) -> tuple[tuple[str, ...], ...]:
    return tuple(row for row in MATRIX_ROWS if row[0] == kind)


PROTOCOL_ROWS = _matrix_rows("protocol")
TYPED_ENVELOPE_ROWS = _matrix_rows("typed-envelope")
EXPECTED_PROTOCOL_IDS = tuple(row[2] for row in PROTOCOL_ROWS)

REMOVED_SNAPSHOT_EXPORTS = (
    "PRIVACY_CAPABILITY_SNAPSHOT_MAX_JSON_BYTES_V1",
    "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
    "PrivacyCapabilityRowV1",
    "PrivacyCapabilitySnapshotError",
    "PrivacyCapabilitySnapshotV1",
    "parse_privacy_capability_snapshot_json_v1",
    "parse_privacy_capability_snapshot_v1",
)

REMOVED_HELPER_MODULES = (
    "anonymous_pgc",
    "jindo",
    "research_adapters",
    "silent_threshold",
    "sis_hints",
    "vega",
    "verange",
    "zk_ams",
    "zk_x509",
    "zkat",
)


def test_protocol_registry_is_exactly_the_canonical_twelve() -> None:
    assert PRIVACY_PROTOCOL_IDS_V1 == EXPECTED_PROTOCOL_IDS
    assert len(PRIVACY_PROTOCOL_IDS_V1) == 12
    assert len(set(PRIVACY_PROTOCOL_IDS_V1)) == 12


def test_shared_matrix_has_no_legacy_or_retirement_namespace() -> None:
    assert MATRIX_TEXT.endswith("\n")
    assert "\r" not in MATRIX_TEXT
    assert all(MATRIX_TEXT.split("\n")[:-1])
    assert {row[0] for row in MATRIX_ROWS} == {
        "matrix-version",
        "registry-sha256",
        "protocol",
        "typed-envelope",
    }
    assert _matrix_rows("matrix-version") == (("matrix-version", "1"),)
    assert len(PROTOCOL_ROWS) == 12
    assert tuple(row[1] for row in PROTOCOL_ROWS) == tuple(map(str, range(12)))
    assert all(len(row) == 5 for row in PROTOCOL_ROWS)
    registry_preimage = "".join(f"{protocol}\n" for protocol in EXPECTED_PROTOCOL_IDS)
    assert _matrix_rows("registry-sha256") == (
        ("registry-sha256", hashlib.sha256(registry_preimage.encode()).hexdigest()),
    )
    assert len(TYPED_ENVELOPE_ROWS) == 12
    assert tuple(row[1:4] for row in TYPED_ENVELOPE_ROWS) == tuple(
        row[2:5] for row in PROTOCOL_ROWS
    )
    assert all(
        len(row) == 6
        and all(
            len(digest) == 64
            and digest == digest.lower()
            and set(digest) <= set("0123456789abcdef")
            and digest != "0" * 64
            for digest in row[4:]
        )
        for row in TYPED_ENVELOPE_ROWS
    )


def test_package_root_uses_native_manifest_not_old_json_snapshot() -> None:
    assert iroha_python.PRIVACY_PROTOCOL_IDS_V1 == PRIVACY_PROTOCOL_IDS_V1
    assert "PRIVACY_PROTOCOL_IDS_V1" in iroha_python.__all__
    assert hasattr(iroha_python, "PrivacyExact12CapabilityManifestV1")
    assert hasattr(iroha_python, "privacy_exact12_capability_manifest_v1")
    for name in REMOVED_SNAPSHOT_EXPORTS:
        assert not hasattr(iroha_python, name)
        assert name not in iroha_python.__all__


def test_removed_free_form_helper_modules_are_absent() -> None:
    for module in REMOVED_HELPER_MODULES:
        with pytest.raises(ModuleNotFoundError):
            importlib.import_module(f"iroha_python.{module}")
