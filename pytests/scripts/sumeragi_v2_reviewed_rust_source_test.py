"""Focused recursive-closure controls for reviewed Sumeragi Rust sources."""

from __future__ import annotations

import os
from pathlib import Path
import sys

import pytest

from pytests.scripts.sumeragi_v2_multilane_models_test import (
    initialize_git_fixture,
    load_checker,
)


def write_reviewed_rust_fixture(
    tmp_path: Path,
    files: dict[str, str],
    tracked: tuple[str, ...] | None = None,
) -> Path:
    root = tmp_path / "repo"
    root.mkdir()
    for relative, source in files.items():
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(source, encoding="utf-8")
    initialize_git_fixture(root, tracked)
    return root


def reviewed_rust_source_module():
    load_checker()
    return sys.modules["sumeragi_v2_multilane_reviewed_rust_source"]


def test_reviewed_rust_source_recursively_expands_grandchild_with_provenance(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": (
                "fn root_before() {}\n"
                'include!("child.rs");\n'
                "fn root_after() {}\n"
            ),
            "src/child.rs": (
                "fn child_before() {}\n"
                'include!("nested/grandchild.rs");\n'
            ),
            "src/nested/grandchild.rs": "fn reviewed_grandchild() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "recursive fixture", errors
    )

    assert errors == []
    assert closure is not None
    assert closure.providers == (
        Path("src/root.rs"),
        Path("src/child.rs"),
        Path("src/nested/grandchild.rs"),
    )
    assert closure.source.count("fn reviewed_grandchild()") == 1
    assert tuple(
        (edge.parent, edge.provider, edge.line, edge.chain)
        for edge in closure.provenance
    ) == (
        (
            Path("src/root.rs"),
            Path("src/child.rs"),
            2,
            (Path("src/root.rs"), Path("src/child.rs")),
        ),
        (
            Path("src/child.rs"),
            Path("src/nested/grandchild.rs"),
            2,
            (
                Path("src/root.rs"),
                Path("src/child.rs"),
                Path("src/nested/grandchild.rs"),
            ),
        ),
    )
    assert "parent=src/child.rs provider=src/nested/grandchild.rs line=2" in (
        closure.source
    )
    manifest_errors: list[str] = []
    expanded = helper._expanded_source_manifest_paths(
        {Path("src/root.rs")}, root, manifest_errors
    )
    assert manifest_errors == []
    assert Path("src/nested/grandchild.rs") in expanded


def test_reviewed_rust_source_rejects_nested_dynamic_include(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": 'include!("child.rs");\n',
            "src/child.rs": 'include!(concat!("nested/", "grandchild.rs"));\n',
            "src/nested/grandchild.rs": "fn hidden_grandchild() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "dynamic fixture", errors
    )

    assert closure is None
    assert any(
        "src/child.rs:1" in error
        and "path must be one literal canonical .rs string" in error
        for error in errors
    ), errors


def test_reviewed_rust_source_rejects_recursive_cycle(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": 'include!("child.rs");\n',
            "src/child.rs": 'include!("loop.rs");\n',
            "src/loop.rs": 'include!("child.rs");\n',
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "cycle fixture", errors
    )

    assert closure is None
    assert any(
        "reviewed Rust include cycle" in error
        and "src/child.rs -> src/loop.rs -> src/child.rs" in error
        for error in errors
    ), errors


def test_reviewed_rust_source_rejects_untracked_nested_provider(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": 'include!("child.rs");\n',
            "src/child.rs": 'include!("grandchild.rs");\n',
            "src/grandchild.rs": "fn untracked_grandchild() {}\n",
        },
        tracked=("src/root.rs", "src/child.rs"),
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "untracked fixture", errors
    )

    assert closure is None
    assert any(
        "src/grandchild.rs" in error
        and "exactly one stage-zero Git index entry" in error
        for error in errors
    ), errors


def test_reviewed_rust_source_rejects_nonregular_nested_provider(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": 'include!("child.rs");\n',
            "src/child.rs": 'include!("provider.rs");\n',
        },
    )
    (root / "src/provider.rs").mkdir()
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "nonregular fixture", errors
    )

    assert closure is None
    assert any(
        "src/provider.rs" in error and "regular non-symlink file" in error
        for error in errors
    ), errors


def test_reviewed_rust_source_rejects_duplicate_provider(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": 'include!("child.rs");\ninclude!("child.rs");\n',
            "src/child.rs": "fn duplicate_provider() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs", "child.rs")},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "duplicate fixture", errors
    )

    assert closure is None
    assert any("duplicate reviewed Rust include provider" in error for error in errors)


def test_reviewed_rust_source_rejects_noncanonical_path_alias(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": 'include!("./child.rs");\n',
            "src/child.rs": "fn path_alias_provider() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("./child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "path alias fixture", errors
    )

    assert closure is None
    assert any(
        "include! path is unsafe or noncanonical" in error for error in errors
    ), errors


def test_reviewed_rust_source_rejects_hardlink_provider_alias(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = tmp_path / "repo"
    (root / "src").mkdir(parents=True)
    (root / "src/root.rs").write_text(
        'include!("first.rs");\ninclude!("second.rs");\n',
        encoding="utf-8",
    )
    first = root / "src/first.rs"
    first.write_text("fn aliased_provider() {}\n", encoding="utf-8")
    os.link(first, root / "src/second.rs")
    initialize_git_fixture(root)
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("first.rs", "second.rs")},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "alias fixture", errors
    )

    assert closure is None
    assert any(
        "provider aliases the same filesystem object" in error for error in errors
    ), errors
