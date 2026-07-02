"""Tests for shared script path-safety helpers."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

import pytest


SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from path_safety import first_symlinked_existing_path_component  # noqa: E402


def _system_symlink_prefix() -> Path:
    prefix = Path("/tmp")
    if not prefix.is_symlink():
        pytest.skip("/tmp is not a symlink on this platform")
    return prefix


def test_first_absolute_component_symlink_prefix_is_allowed() -> None:
    """macOS /tmp-style root aliases must not block regular files."""

    prefix = _system_symlink_prefix()
    with tempfile.TemporaryDirectory(
        prefix="iroha-path-safety-",
        dir=str(prefix),
    ) as directory:
        evidence = Path(directory) / "evidence.txt"
        evidence.write_text("evidence\n", encoding="utf-8")

        assert first_symlinked_existing_path_component(evidence) is None


def test_nested_symlink_under_absolute_prefix_is_rejected() -> None:
    """Symlinks inside the evidence tree remain unsafe."""

    prefix = _system_symlink_prefix()
    with tempfile.TemporaryDirectory(
        prefix="iroha-path-safety-",
        dir=str(prefix),
    ) as directory:
        root = Path(directory)
        real_parent = root / "real"
        real_parent.mkdir()
        (real_parent / "evidence.txt").write_text("evidence\n", encoding="utf-8")
        parent_link = root / "parent-link"
        try:
            parent_link.symlink_to(real_parent, target_is_directory=True)
        except OSError as error:
            pytest.skip(f"symlink creation unavailable: {error}")

        assert (
            first_symlinked_existing_path_component(parent_link / "evidence.txt")
            == parent_link
        )


def test_missing_tail_stops_after_allowed_absolute_prefix() -> None:
    """Missing path suffixes should not convert a root alias into a rejection."""

    prefix = _system_symlink_prefix()
    with tempfile.TemporaryDirectory(
        prefix="iroha-path-safety-",
        dir=str(prefix),
    ) as directory:
        missing = Path(directory) / "missing" / "evidence.txt"

        assert first_symlinked_existing_path_component(missing) is None
