"""Tests for shared SoraFS archive path-component helpers."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_archive_path_components import (  # noqa: E402
    decoded_text_variants,
    path_component_has_uri_scheme_prefix,
    path_component_has_windows_drive_prefix,
)


def test_decoded_text_variants_preserves_stable_raw_text() -> None:
    assert decoded_text_variants("lane-summary.json") == ("lane-summary.json",)


@pytest.mark.parametrize(
    ("encoded", "expected"),
    (
        ("%252e%252e", ("%252e%252e", "%2e%2e", "..")),
        ("&amp;amp;#x2e;", ("&amp;amp;#x2e;", "&amp;#x2e;", "&#x2e;", ".")),
    ),
)
def test_decoded_text_variants_repeatedly_decodes(
    encoded: str, expected: tuple[str, ...]
) -> None:
    assert decoded_text_variants(encoded) == expected


def test_decoded_text_variants_has_a_fixed_round_bound() -> None:
    encoded = "%25252525252e"
    variants = decoded_text_variants(encoded)

    assert variants[0] == encoded
    assert len(variants) == 5
    assert len(set(variants)) == len(variants)


@pytest.mark.parametrize("component", ("C:", "z:lane", "A:/archive"))
def test_windows_drive_prefix_is_recognized(component: str) -> None:
    assert path_component_has_windows_drive_prefix(component)


@pytest.mark.parametrize("component", ("", ":", "1:lane", "lane:archive"))
def test_non_drive_prefix_is_rejected(component: str) -> None:
    assert not path_component_has_windows_drive_prefix(component)


@pytest.mark.parametrize(
    "component",
    ("https:archive", "git+ssh:archive", "a.b-c:archive", "C:archive"),
)
def test_uri_scheme_prefix_is_recognized(component: str) -> None:
    assert path_component_has_uri_scheme_prefix(component)


@pytest.mark.parametrize("component", ("", ":archive", "1http:archive", "archive"))
def test_non_uri_scheme_prefix_is_rejected(component: str) -> None:
    assert not path_component_has_uri_scheme_prefix(component)
