"""Tests for the affected-package Rust CI router."""

from __future__ import annotations

import importlib.util
import json
import sys
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
                "features": {
                    "default": ["portable"],
                    "portable": [],
                    **({"cuda": []} if name == "node" else {}),
                },
                "dependencies": [],
            }
        )
    packages[1]["dependencies"] = [_dependency(root, "base")]
    packages[2]["dependencies"] = [_dependency(root, "node")]
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


def _dependency(
    root: Path,
    package: str,
    *,
    alias: str | None = None,
    optional: bool = False,
    uses_default_features: bool = True,
    features: tuple[str, ...] = (),
    kind: str | None = None,
    target: str | None = None,
) -> dict[str, Any]:
    """Return one synthetic workspace dependency record."""

    return {
        "name": package,
        "source": None,
        "req": "*",
        "kind": kind,
        "rename": alias if alias != package else None,
        "optional": optional,
        "uses_default_features": uses_default_features,
        "features": list(features),
        "target": target,
        "path": str(root / "crates" / package),
    }


def _package(metadata: dict[str, Any], name: str) -> dict[str, Any]:
    """Return a named synthetic Cargo package record."""

    return next(package for package in metadata["packages"] if package["name"] == name)


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


def _manifest_with_exclusions(
    exclusions: dict[str, Any],
) -> Any:
    """Return the synthetic manifest with feature exclusions attached."""

    manifest = _manifest()
    return rust_ci.LaneManifest(
        lanes=manifest.lanes,
        generated_patterns=manifest.generated_patterns,
        all_patterns=manifest.all_patterns,
        ignore_patterns=manifest.ignore_patterns,
        lane_patterns=manifest.lane_patterns,
        feature_exclusions=exclusions,
    )


def test_checked_in_manifest_exhaustively_maps_locked_workspace() -> None:
    """Every current workspace member belongs to exactly one checked-in lane."""

    metadata = rust_ci.load_cargo_metadata(root=ROOT)
    packages = rust_ci.workspace_packages(metadata, root=ROOT)
    manifest = rust_ci.load_lane_manifest()
    rust_ci.validate_manifest(manifest, packages)
    assert len(packages) == sum(len(packages) for packages in manifest.lanes.values())
    assert {
        package: exclusion.features
        for package, exclusion in manifest.feature_exclusions.items()
    } == {
        "connect_norito_bridge": ("cuda",),
        "iroha_audio": ("libopus",),
        "iroha_core": (
            "cuda",
            "kagemusha-candidate-source-seal",
            "kaigi_privacy_mocks",
        ),
        "irohad": ("accel-cuda", "beep"),
        "ivm": ("beep", "cuda", "htm"),
    }


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


def test_qualification_only_feature_is_excluded_without_narrowing_others(
    tmp_path: Path,
) -> None:
    """Generic lanes retain portable coverage while isolating excluded roots."""

    metadata = _metadata(tmp_path)
    _package(metadata, "node")["features"]["beep"] = []
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    commands = rust_ci.commands_for_checks(
        ("base", "node"),
        ("clippy", "doc"),
        feature_exclusions={
            "node": rust_ci.FeatureExclusion(
                ("beep", "cuda"), "These roots require host qualification"
            )
        },
        workspace=workspace,
    )
    assert len(commands) == 4
    ordinary_clippy, qualified_clippy, ordinary_doc, qualified_doc = commands
    assert ordinary_clippy == [
        "cargo",
        "clippy",
        "--locked",
        "--all-targets",
        "--all-features",
        "-p",
        "base",
        "--",
        "-D",
        "warnings",
    ]
    assert qualified_clippy == [
        "cargo",
        "clippy",
        "--locked",
        "--all-targets",
        "-p",
        "node",
        "--features",
        "portable",
        "--",
        "-D",
        "warnings",
    ]
    assert ordinary_doc == [
        "cargo",
        "doc",
        "--locked",
        "--no-deps",
        "--all-features",
        "-p",
        "base",
    ]
    assert qualified_doc == [
        "cargo",
        "doc",
        "--locked",
        "--no-deps",
        "-p",
        "node",
        "--features",
        "portable",
    ]


def test_default_profile_cannot_reenable_an_excluded_feature(
    tmp_path: Path,
) -> None:
    """Implicit defaults remain part of an excluded package's generic profile."""

    metadata = _metadata(tmp_path)
    _package(metadata, "node")["features"]["portable"] = ["cuda"]
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "node": rust_ci.FeatureExclusion(
                ("cuda",), "CUDA artifacts require hardware qualification"
            )
        }
    )
    with pytest.raises(
        rust_ci.ClassificationError, match=r"re-enabled.*node -> node/cuda"
    ):
        rust_ci.validate_manifest(manifest, workspace)


def test_ordinary_profile_cannot_forward_an_exclusion_through_alias(
    tmp_path: Path,
) -> None:
    """A renamed dependency's strong feature edge cannot bypass exclusions."""

    metadata = _metadata(tmp_path)
    integration = _package(metadata, "integration")
    integration["dependencies"][0]["rename"] = "runtime"
    integration["features"]["gpu"] = ["runtime/cuda"]
    metadata["resolve"]["nodes"][2]["deps"][0]["name"] = "runtime"
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "node": rust_ci.FeatureExclusion(
                ("cuda",), "CUDA artifacts require hardware qualification"
            )
        }
    )

    with pytest.raises(
        rust_ci.ClassificationError,
        match=r"re-enabled.*integration -> node/cuda",
    ):
        rust_ci.validate_manifest(manifest, workspace)


def test_dependency_default_features_participate_in_reachability(
    tmp_path: Path,
) -> None:
    """A non-optional workspace edge enables the target package's default."""

    metadata = _metadata(tmp_path)
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "node": rust_ci.FeatureExclusion(
                ("portable",), "Synthetic host-specific default member"
            )
        }
    )

    with pytest.raises(
        rust_ci.ClassificationError,
        match=r"re-enabled.*integration -> node/portable",
    ):
        rust_ci.validate_manifest(manifest, workspace)


def test_weak_dependency_feature_applies_when_dep_edge_activates_alias(
    tmp_path: Path,
) -> None:
    """`dep:` plus `dep?/feature` is resolved independent of member order."""

    metadata = _metadata(tmp_path)
    integration = _package(metadata, "integration")
    integration["dependencies"][0].update(
        {"rename": "runtime", "optional": True}
    )
    integration["features"]["gpu"] = ["runtime?/cuda", "dep:runtime"]
    # Disabled optional dependencies can be absent from Cargo's resolve edges;
    # the raw path and alias must still preserve this feature relationship.
    metadata["resolve"]["nodes"][2]["deps"] = []
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "node": rust_ci.FeatureExclusion(
                ("cuda",), "CUDA artifacts require hardware qualification"
            )
        }
    )

    with pytest.raises(
        rust_ci.ClassificationError,
        match=r"re-enabled.*integration -> node/cuda",
    ):
        rust_ci.validate_manifest(manifest, workspace)


def test_weak_dependency_feature_does_not_activate_optional_alias(
    tmp_path: Path,
) -> None:
    """A lone `dep?/feature` edge remains weak and does not create leakage."""

    metadata = _metadata(tmp_path)
    integration = _package(metadata, "integration")
    integration["dependencies"][0].update(
        {"rename": "runtime", "optional": True}
    )
    integration["features"]["gpu"] = ["runtime?/cuda"]
    metadata["resolve"]["nodes"][2]["deps"] = []
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "node": rust_ci.FeatureExclusion(
                ("cuda",), "CUDA artifacts require hardware qualification"
            )
        }
    )

    rust_ci.validate_manifest(manifest, workspace)


def test_grouped_lane_profile_catches_cross_root_weak_feature_unification(
    tmp_path: Path,
) -> None:
    """Two safe roots cannot jointly activate a weak excluded dependency feature."""

    metadata = _metadata(tmp_path)
    base = _package(metadata, "base")
    base["features"]["blocked"] = []
    node = _package(metadata, "node")
    node["dependencies"][0].update(
        {
            "rename": "qualified",
            "optional": True,
            "uses_default_features": False,
        }
    )
    node["features"].update(
        {
            "activate-qualified": ["dep:qualified"],
            "weak-qualified": ["qualified?/blocked"],
        }
    )
    metadata["resolve"]["nodes"][1]["deps"] = []

    integration = _package(metadata, "integration")
    integration["dependencies"][0].update(
        {
            "uses_default_features": False,
            "features": ["activate-qualified"],
        }
    )
    peer_id = "path+file:///peer#0.1.0"
    metadata["workspace_members"].append(peer_id)
    metadata["packages"].append(
        {
            "id": peer_id,
            "name": "peer",
            "manifest_path": str(tmp_path / "crates" / "peer" / "Cargo.toml"),
            "features": {"default": ["portable"], "portable": []},
            "dependencies": [
                _dependency(
                    tmp_path,
                    "node",
                    uses_default_features=False,
                    features=("weak-qualified",),
                )
            ],
        }
    )
    metadata["resolve"]["nodes"].append(
        {
            "id": peer_id,
            "deps": [{"name": "node", "pkg": metadata["workspace_members"][1]}],
        }
    )
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    blocked = ("base", "blocked")
    integration_profile = {
        "integration": set(workspace["integration"].feature_definitions)
    }
    peer_profile = {"peer": set(workspace["peer"].feature_definitions)}
    assert blocked not in rust_ci._workspace_feature_closure(
        integration_profile, workspace
    )
    assert blocked not in rust_ci._workspace_feature_closure(peer_profile, workspace)
    assert blocked in rust_ci._workspace_feature_closure(
        integration_profile | peer_profile, workspace
    )

    manifest = rust_ci.LaneManifest(
        lanes={
            "foundation": ("base",),
            "node": ("node",),
            "integration": ("integration", "peer"),
        },
        generated_patterns=("target/**", "**/target/**"),
        all_patterns=("Cargo.toml", "fixtures/**"),
        ignore_patterns=("docs/**", "specs/**"),
        lane_patterns={"foundation": (), "node": (), "integration": ()},
        feature_exclusions={
            "base": rust_ci.FeatureExclusion(
                ("blocked",), "Synthetic host-specific dependency feature"
            )
        },
    )
    with pytest.raises(
        rust_ci.ClassificationError,
        match=(
            r"re-enabled.*integration lane \[integration, peer\] "
            r"-> base/blocked"
        ),
    ):
        rust_ci.validate_manifest(manifest, workspace)


@pytest.mark.parametrize("kind", (None, "dev", "build"))
def test_fixed_features_on_target_specific_dependency_kinds_fail_closed(
    tmp_path: Path, kind: str | None
) -> None:
    """Fixed features from every all-target dependency record are reachable."""

    metadata = _metadata(tmp_path)
    _package(metadata, "integration")["dependencies"][0].update(
        {
            "features": ["cuda"],
            "kind": kind,
            "target": 'cfg(target_os = "qualification-host")',
        }
    )
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "node": rust_ci.FeatureExclusion(
                ("cuda",), "CUDA artifacts require hardware qualification"
            )
        }
    )

    with pytest.raises(
        rust_ci.ClassificationError,
        match=r"re-enabled.*integration -> node/cuda",
    ):
        rust_ci.validate_manifest(manifest, workspace)


def test_remaining_feature_of_excluded_package_cannot_leak_other_exclusion(
    tmp_path: Path,
) -> None:
    """Every separately generated excluded-package profile is validated."""

    metadata = _metadata(tmp_path)
    integration = _package(metadata, "integration")
    integration["features"].update(
        {"blocked": [], "portable": ["node/cuda"]}
    )
    workspace = rust_ci.workspace_packages(metadata, root=tmp_path)
    manifest = _manifest_with_exclusions(
        {
            "integration": rust_ci.FeatureExclusion(
                ("blocked",), "Synthetic host-only integration feature"
            ),
            "node": rust_ci.FeatureExclusion(
                ("cuda",), "CUDA artifacts require hardware qualification"
            ),
        }
    )

    with pytest.raises(
        rust_ci.ClassificationError,
        match=r"re-enabled.*integration -> node/cuda",
    ):
        rust_ci.validate_manifest(manifest, workspace)


@pytest.mark.parametrize(
    ("exclusions", "message"),
    (
        (
            {"missing": rust_ci.FeatureExclusion(("cuda",), "Missing package")},
            "absent from workspace",
        ),
        (
            {"node": rust_ci.FeatureExclusion(("missing",), "Missing feature")},
            "absent from Cargo metadata",
        ),
    ),
)
def test_unknown_feature_exclusions_fail_closed(
    tmp_path: Path, exclusions: dict[str, Any], message: str
) -> None:
    """Stale exclusion package and feature names invalidate the manifest."""

    workspace = rust_ci.workspace_packages(_metadata(tmp_path), root=tmp_path)
    with pytest.raises(rust_ci.ClassificationError, match=message):
        rust_ci.validate_manifest(
            _manifest_with_exclusions(exclusions), workspace
        )


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
