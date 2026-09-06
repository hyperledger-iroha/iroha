"""Tests for the affected-package Rust CI router."""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "rust_ci.py"
SPEC = importlib.util.spec_from_file_location("rust_ci", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
rust_ci = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = rust_ci
SPEC.loader.exec_module(rust_ci)


def _metadata(root: Path) -> dict[str, Any]:
    """Return a small Cargo metadata graph: base <- node <- integration."""

    packages = []
    members = []
    for name in ("base", "node", "integration"):
        package_id = f"path+file:///{name}#0.1.0"
        members.append(package_id)
        packages.append(
            {
                "id": package_id,
                "name": name,
                "manifest_path": str(root / "crates" / name / "Cargo.toml"),
            }
        )
    return {
        "workspace_members": members,
        "packages": packages,
        "resolve": {
            "nodes": [
                {"id": members[0], "deps": []},
                {
                    "id": members[1],
                    "deps": [{"name": "base", "pkg": members[0]}],
                },
                {
                    "id": members[2],
                    "deps": [{"name": "node", "pkg": members[1]}],
                },
            ]
        },
    }


def _manifest() -> Any:
    """Return an exhaustive manifest for the synthetic workspace."""

    return rust_ci.LaneManifest(
        lanes={
            "foundation": ("base",),
            "node": ("node",),
            "integration": ("integration",),
        },
        generated_patterns=("target/**", "**/target/**"),
        all_patterns=("Cargo.toml", "fixtures/**"),
        ignore_patterns=("docs/**", "specs/**"),
        lane_patterns={
            "foundation": ("codec/**",),
            "node": (),
            "integration": (),
        },
    )


def test_checked_in_manifest_exhaustively_maps_locked_workspace() -> None:
    """Every current workspace member belongs to exactly one checked-in lane."""

    metadata = rust_ci.load_cargo_metadata(root=ROOT)
    packages = rust_ci.workspace_packages(metadata, root=ROOT)
    manifest = rust_ci.load_lane_manifest()
    rust_ci.validate_manifest(manifest, packages)
    assert len(packages) == sum(len(packages) for packages in manifest.lanes.values())


def test_package_change_expands_reverse_dependency_closure(tmp_path: Path) -> None:
    """A foundation edit selects its node and integration dependants."""

    result = rust_ci.classify_paths(
        ["crates/base/src/lib.rs"],
        metadata=_metadata(tmp_path),
        manifest=_manifest(),
        root=tmp_path,
    )
    assert result.changed_packages == ("base",)
    assert result.impacted_packages == ("base", "integration", "node")
    assert list(result.lane_packages) == ["foundation", "node", "integration"]
    assert result.full is False


def test_deepest_nested_package_owns_path(tmp_path: Path) -> None:
    """Nested proc-macro-style packages override their parent's directory."""

    metadata = _metadata(tmp_path)
    nested_id = "path+file:///nested#0.1.0"
    metadata["workspace_members"].append(nested_id)
    metadata["packages"].append(
        {
            "id": nested_id,
            "name": "nested",
            "manifest_path": str(
                tmp_path / "crates" / "base" / "proc_macro" / "Cargo.toml"
            ),
        }
    )
    metadata["resolve"]["nodes"].append({"id": nested_id, "deps": []})
    manifest = rust_ci.LaneManifest(
        lanes={
            "foundation": ("base", "nested"),
            "node": ("node",),
            "integration": ("integration",),
        },
        generated_patterns=("target/**", "**/target/**"),
        all_patterns=("Cargo.toml",),
        ignore_patterns=("docs/**", "specs/**"),
        lane_patterns={"foundation": (), "node": (), "integration": ()},
    )
    result = rust_ci.classify_paths(
        ["crates/base/proc_macro/src/lib.rs"],
        metadata=metadata,
        manifest=manifest,
        root=tmp_path,
    )
    assert result.changed_packages == ("nested",)


@pytest.mark.parametrize("path", ("new_rust_root/file.rs", "crates/deleted/src/lib.rs"))
def test_unmapped_paths_fail_closed_to_all_packages(
    tmp_path: Path, path: str
) -> None:
    """Unknown or deleted package paths can never silently skip validation."""

    result = rust_ci.classify_paths(
        [path],
        metadata=_metadata(tmp_path),
        manifest=_manifest(),
        root=tmp_path,
    )
    assert result.full is True
    assert result.impacted_packages == ("base", "integration", "node")
    assert result.reasons == (f"unmapped path changed: {path}",)


def test_root_and_shared_inputs_select_all_packages(tmp_path: Path) -> None:
    """Root Cargo and shared fixture changes deliberately select all lanes."""

    result = rust_ci.classify_paths(
        ["fixtures/block.json"],
        metadata=_metadata(tmp_path),
        manifest=_manifest(),
        root=tmp_path,
    )
    assert result.full is True
    assert result.impacted_packages == ("base", "integration", "node")


def test_explicit_non_rust_path_can_skip_rust_lanes(tmp_path: Path) -> None:
    """A documented non-Rust path produces an empty, valid matrix."""

    result = rust_ci.classify_paths(
        ["specs/index.md"],
        metadata=_metadata(tmp_path),
        manifest=_manifest(),
        root=tmp_path,
    )
    assert result.has_rust is False
    assert result.as_dict()["matrix"] == {"include": []}


def test_multiple_lane_path_matches_fail_closed(tmp_path: Path) -> None:
    """Overlapping lane globs cannot silently narrow validation."""

    manifest = _manifest()
    manifest = rust_ci.LaneManifest(
        lanes=manifest.lanes,
        generated_patterns=manifest.generated_patterns,
        all_patterns=manifest.all_patterns,
        ignore_patterns=manifest.ignore_patterns,
        lane_patterns={
            "foundation": ("shared/**",),
            "node": ("shared/config/**",),
            "integration": (),
        },
    )
    result = rust_ci.classify_paths(
        ["shared/config/profile.toml"],
        metadata=_metadata(tmp_path),
        manifest=manifest,
        root=tmp_path,
    )

    assert result.full is True
    assert result.impacted_packages == ("base", "integration", "node")
    assert result.reasons == (
        "ambiguous lane mapping (foundation, node): shared/config/profile.toml",
    )


def test_generated_output_inside_package_never_seeds_compile_lane(
    tmp_path: Path,
) -> None:
    """Ignored build products do not become authored package changes."""

    result = rust_ci.classify_paths(
        ["crates/base/target/debug/generated.rs"],
        metadata=_metadata(tmp_path),
        manifest=_manifest(),
        root=tmp_path,
    )
    assert result.has_rust is False


def test_manifest_rejects_missing_duplicate_and_stale_packages(
    tmp_path: Path,
) -> None:
    """Package ownership drift fails before any affected checks are skipped."""

    broken = rust_ci.LaneManifest(
        lanes={
            "foundation": ("base", "base", "stale"),
            "node": ("node",),
        },
        generated_patterns=("target/**",),
        all_patterns=("Cargo.toml",),
        ignore_patterns=(),
        lane_patterns={"foundation": (), "node": ()},
    )
    with pytest.raises(rust_ci.ClassificationError) as error:
        rust_ci.validate_manifest(
            broken, rust_ci.workspace_packages(_metadata(tmp_path), root=tmp_path)
        )
    message = str(error.value)
    assert "multiple lanes" in message
    assert "missing from lanes" in message
    assert "absent from workspace" in message


def test_cargo_commands_are_locked_package_scoped_and_feature_complete() -> None:
    """Routed lint and docs retain feature coverage without widening package scope."""

    commands = rust_ci.commands_for_checks(
        ("base", "node"), ("clippy", "build", "test", "doc")
    )
    assert len(commands) == 4
    for command in commands:
        assert command[0] == "cargo"
        assert "--locked" in command
        assert "--workspace" not in command
        assert command.count("-p") == 2
    assert commands[0][-3:] == ["--", "-D", "warnings"]
    assert "--all-targets" in commands[0]
    assert "--all-features" in commands[0]
    assert "--no-fail-fast" in commands[2]
    assert "--no-deps" in commands[3]
    assert "--all-features" in commands[3]


def test_cargo_command_rejects_untrusted_package_text() -> None:
    """Package arguments cannot become shell or Cargo option injection."""

    with pytest.raises(rust_ci.ClassificationError, match="invalid Cargo package"):
        rust_ci.commands_for_checks(("base;touch-output",), ("test",))


def test_github_output_is_compact_and_machine_readable(tmp_path: Path) -> None:
    """The workflow output contains a valid dynamic matrix on one line."""

    result = rust_ci.classify_paths(
        ["crates/node/src/lib.rs"],
        metadata=_metadata(tmp_path),
        manifest=_manifest(),
        root=tmp_path,
    )
    output = tmp_path / "github-output"
    rust_ci._write_github_output(output, result)
    values = dict(
        line.split("=", 1)
        for line in output.read_text(encoding="utf-8").splitlines()
    )
    assert values["has_rust"] == "true"
    assert values["full"] == "false"
    matrix = json.loads(values["matrix"])
    assert [entry["lane"] for entry in matrix["include"]] == [
        "node",
        "integration",
    ]


def _manifest_with_binaries() -> Any:
    """Model a binary-free node package and a network integration consumer."""

    return replace(
        _manifest(),
        package_binaries={"integration": ("iroha", "iroha3d")},
        consumers={
            "consistency": rust_ci.BinaryConsumer(("node",), (), ("iroha", "kagami")),
            "kotodama_docs": rust_ci.BinaryConsumer((), ("specs/contracts/**",), ("koto",)),
            "pytests": rust_ci.BinaryConsumer(
                (), ("pytests/network/**",), ("iroha", "iroha3d", "kagami")
            ),
        },
    )


def test_split_matrices_partition_reverse_dependency_closure(tmp_path: Path) -> None:
    """Each selected package runs once, with binaries only for its own needs."""

    result = rust_ci.classify_paths(
        ["crates/base/src/lib.rs"], metadata=_metadata(tmp_path),
        manifest=_manifest_with_binaries(), root=tmp_path,
    )
    document = result.as_dict()
    assert document["binary_free_matrix"]["include"] == [
        {"lane": "foundation", "packages": "base", "package_count": 1},
        {"lane": "node", "packages": "node", "package_count": 1},
    ]
    assert document["binary_matrix"]["include"] == [
        {"lane": "integration", "packages": "integration", "package_count": 1}
    ]
    assert result.binaries == ("iroha", "iroha3d", "kagami")
    assert result.consumers == {"consistency": True, "kotodama_docs": False, "pytests": False}


def test_packages_in_one_lane_can_have_different_binary_requirements(tmp_path: Path) -> None:
    """Sharing a logical lane does not make every package wait for node builds."""

    manifest = replace(
        _manifest_with_binaries(),
        lanes={"node": ("base", "node", "integration")},
        lane_patterns={"node": ()},
    )
    document = rust_ci.classify_paths(
        ["crates/node/src/lib.rs"], metadata=_metadata(tmp_path), manifest=manifest, root=tmp_path,
    ).as_dict()
    assert document["binary_free_matrix"]["include"][0]["packages"] == "node"
    assert document["binary_matrix"]["include"][0]["packages"] == "integration"


@pytest.mark.parametrize("path", ("new/unknown.rs", "Cargo.toml"))
def test_full_and_unknown_changes_select_all_binary_consumers(tmp_path: Path, path: str) -> None:
    """Fail-closed routing selects all consumers and the complete binary union."""

    result = rust_ci.classify_paths(
        [path], metadata=_metadata(tmp_path), manifest=_manifest_with_binaries(), root=tmp_path,
    )
    assert result.full
    assert all(result.consumers.values())
    assert result.binaries == ("iroha", "iroha3d", "kagami", "koto")


@pytest.mark.parametrize(
    ("path", "binaries", "consumer"),
    (("specs/index.md", (), None), ("specs/contracts/example.md", ("koto",), "kotodama_docs"),
     ("pytests/network/test_client.py", ("iroha", "iroha3d", "kagami"), "pytests")),
)
def test_non_rust_inputs_select_only_their_binary_consumer(
    tmp_path: Path, path: str, binaries: tuple[str, ...], consumer: str | None,
) -> None:
    """Documentation and Python changes can select artifacts without Rust lanes."""

    manifest = replace(
        _manifest_with_binaries(), ignore_patterns=("specs/**", "docs/**", "pytests/**")
    )
    result = rust_ci.classify_paths(
        [path], metadata=_metadata(tmp_path), manifest=manifest, root=tmp_path
    )
    assert not result.has_rust
    assert result.binaries == binaries
    assert {name for name, selected in result.consumers.items() if selected} == (
        {consumer} if consumer else set()
    )
    output = tmp_path / "outputs"
    rust_ci._write_github_output(output, result)
    values = dict(line.split("=", 1) for line in output.read_text().splitlines())
    assert values["has_binary_free_rust"] == values["has_binary_rust"] == "false"
    assert values["has_binaries"] == str(bool(binaries)).lower()
    assert json.loads(values["binary_matrix"]) == {"include": []}


def test_checked_in_external_binary_requirements_are_package_scoped() -> None:
    """Only packages that start independent test nodes require release artifacts."""

    manifest = rust_ci.load_lane_manifest()
    assert manifest.package_binaries == {
        name: ("iroha", "iroha3d")
        for name in ("integration_tests", "iroha_test_network", "izanami")
    }
    assert manifest.consumers["consistency"].binaries == ("iroha", "kagami")
    assert manifest.consumers["kotodama_docs"].binaries == ("koto",)


def test_compiler_documentation_consumer_covers_its_complete_source_inventory() -> None:
    """New or existing scanned compiler fences cannot evade the routed docs job."""

    inventory = json.loads((ROOT / "specs/kotodama_v1_docs.json").read_text())
    consumer = rust_ci.load_lane_manifest().consumers["kotodama_docs"]
    assert consumer.kotodama_document_inventory == "specs/kotodama_v1_docs.json"
    assert set(inventory["source_roots"]) == {"docs", "specs"}
    assert not rust_ci._matches("docs/ordinary-prose.md", consumer.paths)
    assert not rust_ci._matches("specs/ordinary-prose.md", consumer.paths)


def _git(root: Path, *arguments: str) -> str:
    """Operate only on a temporary documentation fixture repository."""

    return subprocess.run(
        ["git", *arguments], cwd=root, check=True, capture_output=True, text=True,
    ).stdout.strip()


@pytest.fixture
def executable_docs(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    """Create independent Git history without importing caller configuration."""

    monkeypatch.setenv("GIT_CONFIG_GLOBAL", "/dev/null")
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    _git(tmp_path, "init", "-q")
    _git(tmp_path, "config", "user.name", "Documentation Routing Fixture")
    _git(tmp_path, "config", "user.email", "documentation-routing@example.invalid")
    (tmp_path / "specs").mkdir()
    (tmp_path / "docs").mkdir()
    inventory = "specs/kotodama_v1_docs.json"
    (tmp_path / inventory).write_text(json.dumps({
        "schema": 2, "normative_grammar": "specs/grammar.md",
        "source_documents": ["specs/grammar.md"], "source_roots": ["docs", "specs"],
    }))
    source = "```kotodama\nseiyaku Example {}\n```\n"
    (tmp_path / "specs/grammar.md").write_text(source)
    (tmp_path / "docs/example.md").write_text(source)
    (tmp_path / "docs/prose.md").write_text("# Ordinary prose\n")
    _git(tmp_path, "add", ".")
    _git(tmp_path, "commit", "-qm", "documentation fixture")
    manifest = replace(_manifest_with_binaries(), consumers={
        "kotodama_docs": rust_ci.BinaryConsumer((), (), ("koto",), inventory),
    })
    return tmp_path, manifest, source


@pytest.mark.parametrize("path,contents,selected", [
    ("docs/new.md", "# New prose\n", False),
    ("specs/new.md", "# New prose\n", False),
    ("docs/example.md", "New introduction\n\n```kotodama\nseiyaku Example {}\n```\nAfterword\n", False),
    ("docs/prose.md", None, False),
    ("docs/example.md", None, True),
    ("docs/example.md", "The executable example was removed.\n", True),
    ("docs/example.md", "```kotodama\nseiyaku Changed {}\n```\n", True),
    ("docs/example.md", "```ko zk\nseiyaku Example {}\n```\n", True),
    ("docs/new.mdx", "~~~ko\nseiyaku Added {}\n~~~\n", True),
    ("docs/new.md", "```sh\ncat > example.ko <<'KO'\nseiyaku Shell {}\nKO\n```\n", True),
    ("docs/new.md", "```kotodma\nseiyaku Typo {}\n```\n", True),
    ("docs/new.md", "```\nseiyaku Unlabelled {}\n```\n", True),
    ("docs/new.md", "```kotodama\nseiyaku Unterminated {}\n", True),
    ("docs/new.md", "```kotodama\n\n```\n", True),
    ("docs/history/2026-09-06/old.md", "```kotodama\nmalformed historical evidence\n", False),
    ("docs/history/current-roadmap-coverage.json", "{}", False),
])
def test_executable_document_routing_uses_source_and_git_history(
    executable_docs, path: str, contents: str | None, selected: bool,
) -> None:
    """Actual examples select Koto; unrelated prose/history requests no binaries."""

    root, manifest, _ = executable_docs
    document = root / path
    if contents is None:
        document.unlink()
    else:
        document.parent.mkdir(parents=True, exist_ok=True)
        document.write_text(contents)
    result = rust_ci.classify_paths(
        [path], metadata=_metadata(root), manifest=manifest, root=root,
    )
    assert result.consumers["kotodama_docs"] is selected
    assert result.binaries == (("koto",) if selected else ())
    assert result.has_rust is False
    assert result.full is False


def test_committed_fence_deletion_is_compared_to_supplied_base(executable_docs) -> None:
    """A clean PR checkout must not hide code removed after its merge base."""

    root, manifest, _ = executable_docs
    base = _git(root, "rev-parse", "HEAD")
    (root / "docs/example.md").write_text("Only prose remains.\n")
    _git(root, "add", "docs/example.md")
    _git(root, "commit", "-qm", "remove executable example")
    paths = rust_ci.git_changed_paths(base, root=root)
    assert paths == ("docs/example.md",)
    result = rust_ci.classify_paths(
        paths, metadata=_metadata(root), manifest=manifest, root=root, base_revision=base,
    )
    assert result.binaries == ("koto",)
    assert "executable documentation source changed: docs/example.md" in result.reasons


@pytest.mark.parametrize("problem", ["missing_base", "unreadable_utf8", "escaping_symlink", "inventory_changed"])
def test_documentation_uncertainty_still_selects_compiler(executable_docs, problem: str) -> None:
    """Incomplete evidence cannot turn executable documentation into a skip."""

    root, manifest, _ = executable_docs
    path = "docs/prose.md"
    base = "HEAD"
    if problem == "missing_base":
        base = "missing-fixture-reference"
    elif problem == "unreadable_utf8":
        (root / path).write_bytes(b"\xff")
    elif problem == "escaping_symlink":
        (root / path).unlink()
        (root / path).symlink_to(root.parent / "outside.md")
    else:
        path = "specs/kotodama_v1_docs.json"
    result = rust_ci.classify_paths(
        [path], metadata=_metadata(root), manifest=manifest, root=root, base_revision=base,
    )
    assert result.binaries == ("koto",)
    assert result.reasons


def test_historical_fences_are_not_current_checker_inputs(executable_docs) -> None:
    """Even malformed archived fences remain evidence rather than current source."""

    root, _, _ = executable_docs
    archive = root / "docs/history/2026-09-06/obsolete.md"
    archive.parent.mkdir(parents=True)
    archive.write_text("```kotodama\nthis old fence was never terminated\n")
    checker = rust_ci.KOTODAMA_DOCS
    inventory = checker.load_document_set(root / "specs/kotodama_v1_docs.json", root)
    fences = checker.collect_source_fences(inventory, root)
    assert {fence.document for fence in fences} == {Path("docs/example.md"), Path("specs/grammar.md")}


def test_cli_passes_merge_base_to_executable_document_selection(monkeypatch: pytest.MonkeyPatch) -> None:
    """Path enumeration and source comparison use the same PR revision."""

    args = rust_ci.build_parser().parse_args(["classify", "--base", "fixture-main"])
    monkeypatch.setattr(rust_ci, "load_cargo_metadata", lambda **_: {})
    monkeypatch.setattr(rust_ci, "load_lane_manifest", lambda _: _manifest())
    monkeypatch.setattr(rust_ci, "git_changed_paths", lambda _: ("docs/deleted.md",))
    monkeypatch.setattr(rust_ci, "_run", lambda *_args, **_kwargs: subprocess.CompletedProcess([], 0, "base-commit\n"))
    monkeypatch.setattr(rust_ci, "classify_paths", lambda *args, **kwargs: kwargs)
    assert rust_ci._classification_from_args(args)["base_revision"] == "base-commit"


@pytest.mark.parametrize("inventory", [0, False, "", "../outside.json", "/outside.json", "./specs/docs.json", "specs/docs.md"])
def test_consumer_rejects_invalid_document_inventory_paths(tmp_path: Path, inventory) -> None:
    """A malformed source selector cannot quietly become a prose-only skip."""

    text = rust_ci.DEFAULT_MANIFEST.read_text().replace(
        'kotodama_document_inventory = "specs/kotodama_v1_docs.json"',
        f"kotodama_document_inventory = {json.dumps(inventory)}",
    )
    manifest = tmp_path / "lanes.toml"
    manifest.write_text(text)
    with pytest.raises(rust_ci.ClassificationError):
        rust_ci.load_lane_manifest(manifest)


def test_misspelled_consumer_source_selector_is_rejected(tmp_path: Path) -> None:
    """Unknown consumer fields fail closed instead of disabling source scanning."""

    manifest = tmp_path / "lanes.toml"
    manifest.write_text(rust_ci.DEFAULT_MANIFEST.read_text().replace(
        "kotodama_document_inventory =", "kotodama_doc_inventory =",
    ))
    with pytest.raises(rust_ci.ClassificationError, match="unknown fields"):
        rust_ci.load_lane_manifest(manifest)


@pytest.mark.parametrize("binary", ("unknown", "iroha;touch-output", "--workspace"))
def test_invalid_binary_requirements_fail_before_build(binary: str) -> None:
    """The artifact selection cannot inject package names or Cargo options."""

    with pytest.raises(rust_ci.ClassificationError, match="shipping binaries"):
        rust_ci._binary_names([binary], "test")


def test_unknown_binary_requirement_owner_is_rejected(tmp_path: Path) -> None:
    """A stale consumer package cannot silently stop requesting its binaries."""

    manifest = replace(_manifest(), package_binaries={"removed": ("iroha",)})
    with pytest.raises(rust_ci.ClassificationError, match="unknown packages"):
        rust_ci.validate_manifest(
            manifest, rust_ci.workspace_packages(_metadata(tmp_path), root=tmp_path)
        )


def test_selected_binaries_build_once_and_stage_only_requested_outputs(
    tmp_path: Path, monkeypatch: Any
) -> None:
    """Artifact staging uses one locked build and excludes unrelated executable outputs."""

    calls = []
    def fake_run(command: Any, **kwargs: Any) -> None:
        calls.append((command, kwargs))
        target = tmp_path / "target" / "release"
        target.mkdir(parents=True)
        for name in rust_ci.BINARY_PACKAGES:
            (target / name).write_text(name)
    monkeypatch.setattr(rust_ci, "_run", fake_run)
    output = tmp_path / "artifacts"
    rust_ci.build_binaries(("koto", "iroha"), output, root=tmp_path)
    assert len(calls) == 1
    command, options = calls[0]
    assert command == [
        "cargo", "build", "--locked", "--release",
        "-p", "iroha_cli", "--bin", "iroha", "-p", "ivm", "--bin", "koto",
    ]
    assert options == {"cwd": tmp_path, "capture_output": False}
    assert sorted(path.name for path in output.iterdir()) == ["iroha", "koto"]
    with pytest.raises(rust_ci.ClassificationError, match="already exists"):
        rust_ci.build_binaries(("iroha",), output, root=tmp_path)
    assert len(calls) == 1
