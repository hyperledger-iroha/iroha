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


@pytest.fixture(autouse=True)
def isolate_nested_git_fixture_index(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("GIT_INDEX_FILE", raising=False)


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


def test_reviewed_rust_source_expands_manifest_path_module_in_lexical_order(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": (
                "fn root_before() {}\n"
                '#[path = "../tests/ignored.rs"]\n'
                "mod ignored;\n"
                'include!("first.rs");\n'
                '#[path = "declared.rs"]\n'
                "#[cfg_attr(test, allow(dead_code))]\n"
                "pub(crate) mod declared;\n"
                'include!("last.rs");\n'
            ),
            "src/first.rs": "fn reviewed_first() {}\n",
            "src/declared.rs": (
                "fn reviewed_declared() {}\n"
                '#[path = "../tests/unrelated.rs"]\n'
                "mod unrelated;\n"
                'include!("nested.rs");\n'
            ),
            "src/nested.rs": "fn reviewed_nested() {}\n",
            "src/last.rs": "fn reviewed_last() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {
            "src/root.rs": (
                "first.rs",
                "declared.rs",
                "last.rs",
            )
        },
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "path-module fixture", errors
    )

    assert errors == []
    assert closure is not None
    assert closure.providers == (
        Path("src/root.rs"),
        Path("src/first.rs"),
        Path("src/declared.rs"),
        Path("src/nested.rs"),
        Path("src/last.rs"),
    )
    assert Path("tests/ignored.rs") not in closure.providers
    assert tuple(
        (edge.parent, edge.provider, edge.line) for edge in closure.provenance
    ) == (
        (Path("src/root.rs"), Path("src/first.rs"), 4),
        (Path("src/root.rs"), Path("src/declared.rs"), 5),
        (Path("src/declared.rs"), Path("src/nested.rs"), 4),
        (Path("src/root.rs"), Path("src/last.rs"), 8),
    )
    assert closure.source.index("fn reviewed_first()") < closure.source.index(
        "fn reviewed_declared()"
    )
    assert closure.source.index("fn reviewed_nested()") < closure.source.index(
        "fn reviewed_last()"
    )


def test_reviewed_rust_source_expands_manifest_plain_module_in_lexical_order(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": (
                'include!("first.rs");\n'
                "pub(super) mod plain;\n"
                '#[path = "declared.rs"]\n'
                "mod declared;\n"
                'include!("last.rs");\n'
            ),
            "src/first.rs": "fn reviewed_first() {}\n",
            "src/root/plain.rs": "fn reviewed_plain() {}\n",
            "src/declared.rs": "fn reviewed_declared() {}\n",
            "src/last.rs": "fn reviewed_last() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {
            "src/root.rs": (
                "first.rs",
                "root/plain.rs",
                "declared.rs",
                "last.rs",
            )
        },
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "plain-module fixture", errors
    )

    assert errors == []
    assert closure is not None
    assert closure.providers == (
        Path("src/root.rs"),
        Path("src/first.rs"),
        Path("src/root/plain.rs"),
        Path("src/declared.rs"),
        Path("src/last.rs"),
    )
    assert tuple(
        (edge.provider, edge.line) for edge in closure.provenance
    ) == (
        (Path("src/first.rs"), 1),
        (Path("src/root/plain.rs"), 2),
        (Path("src/declared.rs"), 3),
        (Path("src/last.rs"), 5),
    )
    offsets = tuple(
        closure.source.index(name)
        for name in (
            "fn reviewed_first()",
            "fn reviewed_plain()",
            "fn reviewed_declared()",
            "fn reviewed_last()",
        )
    )
    assert offsets == tuple(sorted(offsets))


def test_reviewed_rust_source_rejects_ambiguous_plain_module_layout(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": "mod child;\n",
            "src/root/child.rs": "fn flat_child() {}\n",
            "src/root/child/mod.rs": "fn directory_child() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {
            "src/root.rs": (
                "root/child.rs",
                "root/child/mod.rs",
            )
        },
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "ambiguous plain-module fixture", errors
    )

    assert closure is None
    assert any(
        "plain module 'child' is ambiguous" in error for error in errors
    ), errors


def test_reviewed_rust_source_does_not_treat_path_module_as_plain_module(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": '#[path = "substitute.rs"]\nmod child;\n',
            "src/substitute.rs": "fn substituted_child() {}\n",
            "src/root/child.rs": "fn reviewed_child() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("root/child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "path-bound plain-module fixture", errors
    )

    assert closure is None
    assert any(
        "reviewed Rust include inventory must equal ('root/child.rs',)" in error
        and "found ()" in error
        for error in errors
    ), errors


def test_reviewed_rust_source_rejects_duplicate_include_and_path_binding(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": (
                'include!("child.rs");\n'
                '#[path = "child.rs"]\n'
                "mod child;\n"
            ),
            "src/child.rs": "fn duplicate_provider() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "duplicate path-module fixture", errors
    )

    assert closure is None
    assert any(
        "duplicate reviewed Rust include provider binding 'child.rs'" in error
        and "via #[path] mod" in error
        and "first bound at line 1 via include!" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("attribute", "diagnostic"),
    (
        (
            '#[path = concat!("child", ".rs")]\n',
            "#[path] path must be one literal canonical .rs string",
        ),
        (
            '#[path = "./child.rs"]\n',
            "#[path] path is unsafe or noncanonical",
        ),
        (
            '#[path = "nested/../child.rs"]\n',
            "#[path] path is unsafe or noncanonical",
        ),
    ),
)
def test_reviewed_rust_source_rejects_unsafe_path_module_binding(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    attribute: str,
    diagnostic: str,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": attribute + "mod child;\n",
            "src/child.rs": "fn hidden_provider() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "unsafe path-module fixture", errors
    )

    assert closure is None
    assert any(diagnostic in error for error in errors), errors


def test_reviewed_rust_source_rejects_missing_manifest_path_binding(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(
        tmp_path,
        {
            "src/root.rs": '#[path = "ignored.rs"]\nmod ignored;\n',
            "src/child.rs": "fn missing_provider() {}\n",
        },
    )
    monkeypatch.setattr(
        helper,
        "_REVIEWED_RUST_INCLUDE_MANIFESTS",
        {"src/root.rs": ("child.rs",)},
    )
    errors: list[str] = []
    closure = helper._resolve_reviewed_rust_source(
        root, "src/root.rs", "missing path-module fixture", errors
    )

    assert closure is None
    assert any(
        "reviewed Rust include inventory must equal ('child.rs',)" in error
        and "found ()" in error
        for error in errors
    ), errors


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
    outer_index = tmp_path / "outer.index"
    outer_index.write_bytes(b"outer-index-sentinel")
    monkeypatch.setenv("GIT_INDEX_FILE", str(outer_index))
    initialize_git_fixture(root)
    assert outer_index.read_bytes() == b"outer-index-sentinel"
    monkeypatch.delenv("GIT_INDEX_FILE")
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
