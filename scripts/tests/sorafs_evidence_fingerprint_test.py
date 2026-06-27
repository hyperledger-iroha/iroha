"""Tests for shared SoraFS evidence fingerprint helpers."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_evidence_fingerprint import artifact_fingerprint  # noqa: E402


def test_artifact_fingerprint_selects_fields_in_order() -> None:
    payload = {
        "schema": "test",
        "digest": "abc",
        "ignored": "raw",
    }

    assert artifact_fingerprint(payload, ("schema", "digest", "missing")) == {
        "schema": "test",
        "digest": "abc",
        "missing": None,
    }


def test_artifact_fingerprint_rejects_non_object_payload_without_traceback() -> None:
    with pytest.raises(ValueError, match="payload must be an object"):
        artifact_fingerprint("payload", ("schema",))


def test_artifact_fingerprint_rejects_string_field_sequence_without_splitting() -> None:
    with pytest.raises(ValueError, match="fields must be a sequence of strings"):
        artifact_fingerprint({"schema": "test"}, "schema")


def test_artifact_fingerprint_rejects_non_string_field_without_traceback() -> None:
    with pytest.raises(ValueError, match="fields must be non-empty strings"):
        artifact_fingerprint({"schema": "test"}, ("schema", 7))


def test_artifact_fingerprint_rejects_blank_field_without_traceback() -> None:
    with pytest.raises(ValueError, match="fields must be non-empty strings"):
        artifact_fingerprint({"schema": "test"}, ("schema", " "))


def test_artifact_fingerprint_rejects_padded_field_without_drift() -> None:
    with pytest.raises(ValueError, match="fields must be canonical strings"):
        artifact_fingerprint({"schema": "test"}, ("schema", " digest"))


def test_artifact_fingerprint_rejects_duplicate_fields_without_overwrite() -> None:
    with pytest.raises(ValueError, match="fields must not contain duplicates"):
        artifact_fingerprint({"schema": "test"}, ("schema", "schema"))
