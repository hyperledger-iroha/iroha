"""Tests for shared SoraFS evidence fingerprint helpers."""

from __future__ import annotations

import sys
from pathlib import Path


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
