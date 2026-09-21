"""Focused recursive-closure controls for reviewed Sumeragi Rust sources."""

from __future__ import annotations

import os
import shutil
import subprocess
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


def warm_fixture(tmp_path, monkeypatch):
    helper = reviewed_rust_source_module()
    root = write_reviewed_rust_fixture(tmp_path, {
        "src/root.rs": 'include!("nested/child.rs");\n',
        "src/nested/child.rs": "fn retained_owner() { admit(); }\n",
    })
    monkeypatch.setattr(helper, "_REVIEWED_RUST_INCLUDE_MANIFESTS", {
        "src/root.rs": ("nested/child.rs",),
    })
    closure, errors = resolve_fresh(helper, root)
    assert closure is not None and errors == []
    return helper, root


def resolve_fresh(helper, root, relative="src/root.rs"):
    errors = []
    with helper._reviewed_rust_source_cache():
        closure = helper._resolve_reviewed_rust_source(root, relative, "warm fixture", errors)
    assert helper._ACTIVE_REVIEWED_RUST_SOURCE_CACHE is None
    assert helper._ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE is None
    return closure, errors


def fixture_git(root, *arguments, input=None):
    environment = os.environ.copy()
    environment.pop("GIT_INDEX_FILE", None)
    return subprocess.run(["git", "-C", str(root), *arguments], input=input,
                          text=True, capture_output=True, check=True, env=environment).stdout.strip()


@pytest.mark.parametrize("mutation,diagnostic", [
    ("untracked", "exactly one stage-zero Git index entry"),
    ("unmerged", "exactly one stage-zero Git index entry"),
    ("index-mode", "non-regular Git mode"),
    ("provider-symlink", "regular non-symlink file"),
    ("ancestor-symlink", "regular non-symlink file"),
    ("missing", "missing or unreadable"),
    ("hardlink", "same filesystem object"),
    ("portable-alias", "aliases previous provider"),
])
def test_warm_text_never_authorizes_invalid_provider_namespace(tmp_path, monkeypatch, mutation, diagnostic):
    helper, root = warm_fixture(tmp_path, monkeypatch)
    child = root / "src/nested/child.rs"
    blob = fixture_git(root, "hash-object", str(child))
    if mutation == "untracked":
        fixture_git(root, "update-index", "--force-remove", "src/nested/child.rs")
    elif mutation == "unmerged":
        fixture_git(root, "update-index", "--index-info", input=(
            f"0 {'0' * len(blob)}\tsrc/nested/child.rs\n"
            f"100644 {blob} 1\tsrc/nested/child.rs\n"
            f"100644 {blob} 2\tsrc/nested/child.rs\n"))
    elif mutation == "index-mode":
        fixture_git(root, "update-index", "--cacheinfo", f"120000,{blob},src/nested/child.rs")
    elif mutation == "provider-symlink":
        actual = child.with_name("actual.rs")
        child.rename(actual)
        child.symlink_to(actual.name)
    elif mutation == "ancestor-symlink":
        original = child.parent
        original.rename(original.with_name("actual"))
        original.symlink_to("actual", target_is_directory=True)
    elif mutation == "missing":
        child.unlink()
    else:
        alias = "other.rs" if mutation == "hardlink" else "CHILD.rs"
        target = child.with_name(alias)
        if mutation == "hardlink":
            os.link(child, target)
        elif not target.exists():
            target.write_bytes(child.read_bytes())
        fixture_git(root, "update-index", "--add", "--cacheinfo", f"100644,{blob},src/nested/{alias}")
        (root / "src/root.rs").write_text(f'include!("nested/child.rs");\ninclude!("nested/{alias}");\n')
        monkeypatch.setattr(helper, "_REVIEWED_RUST_INCLUDE_MANIFESTS", {
            "src/root.rs": ("nested/child.rs", f"nested/{alias}"),
        })
    closure, errors = resolve_fresh(helper, root)
    assert closure is None
    assert any(diagnostic in error for error in errors), errors
    assert any(str(root) in error for error in errors), errors


@pytest.mark.parametrize("replace_inode", [False, True])
def test_warm_same_size_restored_mtime_mutation_reaches_source_binding_refusal(tmp_path, monkeypatch, replace_inode):
    helper, root = warm_fixture(tmp_path, monkeypatch)
    checker = load_checker()
    formal = root / "formal"
    formal.mkdir()
    (formal / "Warm.tla").write_text("---- MODULE Warm ----\nRefines == TRUE\nBound == TRUE\n====\n")
    (formal / "warm_fixed.cfg").write_text("INIT Init\nNEXT Next\nINVARIANT Bound\n")
    (formal / "warm_bug.cfg").write_text("INVARIANT Bound\n")
    model = {"module": "Warm", "positive_config": "warm_fixed.cfg", "production_refinement_obligation": "Refines",
             "mutations": [{"config": "warm_bug.cfg", "invariant": "Bound"}],
             "production_symbols": [{"path": "src/root.rs", "kind": "fn", "symbol": "retained_owner", "required_tokens": ["admit();"]}]}
    def validate():
        errors = []
        with helper._reviewed_rust_source_cache():
            checker._validate_model(root, formal, model, errors, reviewed_invariants=("Bound",))
        return errors
    assert validate() == []
    child = root / "src/nested/child.rs"
    metadata = child.stat()
    original = child.read_bytes()
    changed = original.replace(b"admit", b"evade")
    assert len(changed) == len(original) and changed != original
    if replace_inode:
        other = child.with_name("replacement.rs")
        other.write_bytes(changed)
        other.replace(child)
        assert child.stat().st_ino != metadata.st_ino
    else:
        child.write_bytes(changed)
        assert child.stat().st_ino == metadata.st_ino
    os.utime(child, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
    errors = validate()
    assert len(errors) == 1 and "missing source-binding token 'admit();'" in errors[0], errors
    child.write_bytes(original)
    assert validate() == [], "a previous refusal cannot poison repaired current source"


@pytest.mark.parametrize("mutation,diagnostic", [
    ("manifest", "reviewed Rust include inventory must equal"),
    ("removed", "reviewed Rust include inventory must equal"),
    ("duplicate", "duplicate reviewed Rust include provider"),
    ("dynamic", "must be one literal canonical .rs string"),
    ("cycle", "reviewed Rust include cycle"),
])
def test_warm_text_keeps_current_include_inventory_and_parser_refusals(tmp_path, monkeypatch, mutation, diagnostic):
    helper, root = warm_fixture(tmp_path, monkeypatch)
    parent = root / "src/root.rs"
    if mutation == "manifest":
        monkeypatch.setattr(helper, "_REVIEWED_RUST_INCLUDE_MANIFESTS", {"src/root.rs": ("different.rs",)})
    elif mutation == "removed":
        parent.write_text("fn root_without_child() {}\n")
    elif mutation == "duplicate":
        parent.write_text(parent.read_text() * 2)
    elif mutation == "dynamic":
        parent.write_text('include!(concat!("nested/", "child.rs"));\n')
    else:
        child = root / "src/nested/child.rs"
        child.write_text('include!("loop.rs");\n')
        (child.parent / "loop.rs").write_text('include!("child.rs");\n')
        fixture_git(root, "add", "src/nested/loop.rs")
    closure, errors = resolve_fresh(helper, root)
    assert closure is None
    assert any(diagnostic in error for error in errors), errors


def test_warm_refusal_diagnostics_keep_current_path_and_context_cleanup(tmp_path, monkeypatch):
    helper, root = warm_fixture(tmp_path, monkeypatch)
    with pytest.raises(RuntimeError, match="fixture unwind"):
        with helper._reviewed_rust_source_cache():
            raise RuntimeError("fixture unwind")
    assert helper._ACTIVE_REVIEWED_RUST_SOURCE_CACHE is None
    assert helper._ACTIVE_REVIEWED_RUST_GIT_INDEX_CACHE is None
    assert resolve_fresh(helper, root)[1] == []
    invalid = 'include!(env!("UNTRUSTED"));\n'
    for label in ("first", "second"):
        path = root / f"src/{label}.rs"
        path.write_text(invalid)
        fixture_git(root, "add", str(path))
        monkeypatch.setattr(helper, "_REVIEWED_RUST_INCLUDE_MANIFESTS", {f"src/{label}.rs": ()})
        closure, errors = resolve_fresh(helper, root, f"src/{label}.rs")
        assert closure is None
        assert any(str(path) in error for error in errors), errors
        if label == "second":
            assert all("src/first.rs" not in error for error in errors)


def copied_helper_tree(tmp_path):
    helper = reviewed_rust_source_module()
    root = tmp_path / "checker"
    for relative in (helper.REVIEWED_RUST_SOURCE_HELPER_RELATIVE,
                     helper.REVIEWED_RUST_TEXT_HELPER_RELATIVE,
                     helper.REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE):
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(helper.DEFAULT_ROOT / relative, destination)
    return helper, root


LOADER_PREFIX = '''
import importlib.util, json, os, pathlib, sys, types
root = pathlib.Path(sys.argv[1])
resolver_path = root / "scripts/formal/sumeragi_v2_multilane_reviewed_rust_source.py"
text_path = root / "scripts/formal/sumeragi_v2_rust_text.py"
def load(alias="isolated_reviewed_source"):
    spec = importlib.util.spec_from_file_location(alias, resolver_path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[alias] = module
    spec.loader.exec_module(module)
    return module
'''


def isolated_loader(root, body):
    return subprocess.run([sys.executable, "-I", "-S", "-c", LOADER_PREFIX + body, str(root)],
                          text=True, capture_output=True, check=False)


def test_clean_standalone_aliases_share_only_authenticated_pure_text(tmp_path):
    _, root = copied_helper_tree(tmp_path)
    result = isolated_loader(root, '''
a = load("first_resolver")
b = load("second_resolver")
assert a._RUST_TEXT_HELPER is b._RUST_TEXT_HELPER
pure = a._RUST_TEXT_HELPER
pure._clear_mask_cache()
source = "fn owner() { /* secret comment */ execute(); }"
first = a._mask_rust_comments(source)
info = pure._mask_cache_info()
second = b._mask_rust_comments(source.encode().decode())
after = pure._mask_cache_info()
assert first is second
assert after["misses"] == info["misses"] and after["hits"] > info["hits"]
assert "secret comment" not in first and len(first) == len(source)
''')
    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize("operation", ["context", "manifest"])
def test_same_path_executed_helper_change_refuses_even_with_restored_metadata(tmp_path, operation):
    _, root = copied_helper_tree(tmp_path)
    result = isolated_loader(root, f'''
helper = load()
helper._mask_rust_comments("fn warm() {{}}")
metadata = text_path.stat()
original = text_path.read_bytes()
changed = original.replace(b"\\n", b"\\r", 1)
assert len(changed) == len(original) and changed != original
text_path.write_bytes(changed)
os.utime(text_path, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
try:
    if {operation!r} == "context":
        with helper._reviewed_rust_source_cache():
            pass
    else:
        helper._expanded_source_manifest_paths(set(), root=root)
except RuntimeError as error:
    assert "reviewed Rust text helper" in str(error) and "executed source changed" in str(error), str(error)
else:
    raise AssertionError("changed executed helper admitted")
''')
    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize("metadata", ["foreign", "same-origin", "forged-executed-record"])
def test_preloaded_foreign_text_helper_is_never_accepted(tmp_path, metadata):
    _, root = copied_helper_tree(tmp_path)
    result = isolated_loader(root, f'''
foreign = types.ModuleType("_iroha_sumeragi_v2_rust_text")
foreign.__file__ = str(root / "foreign.py" if {metadata!r} == "foreign" else text_path)
if {metadata!r} == "forged-executed-record":
    import hashlib
    foreign._executed_source_origin = str(text_path)
    foreign._executed_source_sha256 = hashlib.sha256(text_path.read_bytes()).hexdigest()
foreign._mask_rust_comments = lambda source: source
sys.modules[foreign.__name__] = foreign
try:
    load()
except RuntimeError as error:
    assert "foreign or unauthenticated module" in str(error), str(error)
else:
    raise AssertionError("preloaded foreign helper admitted")
''')
    assert result.returncode == 0, result.stdout + result.stderr


def test_stale_timestamp_pyc_cannot_replace_executed_source_bytes(tmp_path):
    _, root = copied_helper_tree(tmp_path)
    result = isolated_loader(root, r'''
import py_compile
source = text_path.read_bytes()
metadata = text_path.stat()
evil = b'raise AssertionError("STALE PYC EXECUTED")\n'
assert len(evil) < len(source)
text_path.write_bytes(evil + b" " * (len(source) - len(evil)))
os.utime(text_path, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
py_compile.compile(str(text_path), doraise=True)
text_path.write_bytes(source)
os.utime(text_path, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
helper = load()
helper._validate_executed_rust_text_helper()
assert "hidden" not in helper._mask_rust_comments("/* hidden */ fn visible() {}")
''')
    assert result.returncode == 0, result.stdout + result.stderr


@pytest.mark.parametrize("mutation", ["missing", "symlink", "changed"])
def test_complete_source_closure_rejects_substituted_copied_helper(tmp_path, mutation):
    _, root = copied_helper_tree(tmp_path)
    result = isolated_loader(root, f'''
import shutil
helper = load()
copy = root / "input"
relative = helper.REVIEWED_RUST_TEXT_HELPER_RELATIVE
copied = copy / relative
copied.parent.mkdir(parents=True)
shutil.copy2(text_path, copied)
helper._validate_executed_rust_text_helper(copy)
if {mutation!r} == "missing":
    copied.unlink()
elif {mutation!r} == "symlink":
    copied.unlink()
    copied.symlink_to(text_path)
else:
    copied.write_bytes(copied.read_bytes() + b"\\n# changed copied helper\\n")
try:
    helper._expanded_source_manifest_paths({{helper.REVIEWED_RUST_SOURCE_HELPER_RELATIVE}}, root=copy)
except RuntimeError as error:
    assert "reviewed Rust text helper" in str(error), str(error)
else:
    raise AssertionError("invalid copied text helper admitted")
''')
    assert result.returncode == 0, result.stdout + result.stderr


def test_identical_warm_text_across_roots_still_reads_each_source_and_namespace(tmp_path, monkeypatch):
    helper, first = warm_fixture(tmp_path, monkeypatch)
    second_parent = tmp_path / "other"
    second_parent.mkdir()
    second = write_reviewed_rust_fixture(second_parent, {
        "src/root.rs": (first / "src/root.rs").read_text(),
        "src/nested/child.rs": (first / "src/nested/child.rs").read_text(),
    })
    pure = helper._RUST_TEXT_HELPER
    pure._clear_mask_cache()
    index_reads, stat_reads, payload_reads = [], [], []
    original_index = helper._load_git_index
    original_stat = helper._strict_provider_stat
    original_read = Path.read_text
    def index(root, errors):
        index_reads.append(root)
        return original_index(root, errors)
    def stat(root, relative, inventory, errors):
        stat_reads.append((root, relative))
        return original_stat(root, relative, inventory, errors)
    def read(path, *args, **kwargs):
        if path.suffix == ".rs":
            payload_reads.append(path)
        return original_read(path, *args, **kwargs)
    monkeypatch.setattr(helper, "_load_git_index", index)
    monkeypatch.setattr(helper, "_strict_provider_stat", stat)
    monkeypatch.setattr(Path, "read_text", read)
    assert resolve_fresh(helper, first)[1] == []
    cold = pure._mask_cache_info()
    assert resolve_fresh(helper, first)[1] == []
    assert resolve_fresh(helper, second)[1] == []
    warm = pure._mask_cache_info()
    assert warm["misses"] == cold["misses"]
    assert warm["hits"] > cold["hits"]
    assert index_reads == [first, first, second]
    assert len(stat_reads) == 6
    assert len(payload_reads) == 6
    assert {root for root, _ in stat_reads} == {first, second}
    assert {path for path in payload_reads} == {
        root / relative for root in (first, second)
        for relative in ("src/root.rs", "src/nested/child.rs")
    }
