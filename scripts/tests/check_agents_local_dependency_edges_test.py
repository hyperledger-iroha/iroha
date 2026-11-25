"""Mutations of the narrow existing-local-workspace lockfile exception."""
from __future__ import annotations

import copy
from pathlib import Path

import pytest

import check_agents_local_dependency_edges as guard


def fixture(root):
    before = {"version": 4, "package": [
        {"name": "xtask", "version": "1.0.0", "dependencies": ["external"]},
        {"name": "kernel", "version": "1.0.0"},
        {"name": "external", "version": "2.0.0", "source": "registry+https://example.invalid/index", "checksum": "a" * 64}]}
    after = copy.deepcopy(before)
    after["package"][0]["dependencies"].append("kernel")
    xtask = {"package": {"name": "xtask", "version": "1.0.0"}, "dependencies": {"external": "2.0.0"}}
    updated = copy.deepcopy(xtask)
    updated["dependencies"]["kernel"] = {"path": "../crates/kernel"}
    kernel = {"package": {"name": "kernel", "version": "1.0.0"}}
    manifests = {"xtask/Cargo.toml": (xtask, updated), "crates/kernel/Cargo.toml": (kernel, copy.deepcopy(kernel))}
    dependency = {"name": "kernel", "source": None, "kind": None, "target": None, "rename": None,
                  "optional": False, "features": [], "uses_default_features": True,
                  "path": str(root / "crates/kernel")}
    metadata = {"workspace_members": ["xtask-id", "kernel-id"], "packages": [
        {"name": "xtask", "version": "1.0.0", "id": "xtask-id", "source": None,
         "manifest_path": str(root / "xtask/Cargo.toml"), "dependencies": [dependency]},
        {"name": "kernel", "version": "1.0.0", "id": "kernel-id", "source": None,
         "manifest_path": str(root / "crates/kernel/Cargo.toml"), "dependencies": []}]}
    return before, after, manifests, metadata


def test_existing_workspace_edge_requires_both_manifest_and_lock(tmp_path):
    root = tmp_path.resolve()
    values = fixture(root)
    assert guard.validate(*values, root) == {"xtask/Cargo.toml": ["kernel"]}


@pytest.mark.parametrize("mutation", ["checksum", "version", "source", "new-package", "remove-package",
                                     "remove-edge", "external-edge", "header", "duplicate-edge", "self-edge"])
def test_other_lock_changes_still_refuse(tmp_path, mutation):
    root = tmp_path.resolve()
    before, after, manifests, metadata = fixture(root)
    if mutation in ("checksum", "version", "source"):
        after["package"][2][mutation] = "changed"
    elif mutation == "new-package":
        after["package"].append({"name": "new", "version": "1"})
    elif mutation == "remove-package":
        after["package"].pop()
    elif mutation == "remove-edge":
        after["package"][0]["dependencies"].remove("external")
    elif mutation == "external-edge":
        before["package"][0]["dependencies"] = []
    elif mutation == "header":
        after["version"] = 3
    elif mutation == "duplicate-edge":
        after["package"][0]["dependencies"].append("kernel")
    else:
        after["package"][0]["dependencies"].append("xtask")
    with pytest.raises(guard.EdgeError):
        guard.validate(before, after, manifests, metadata, root)


@pytest.mark.parametrize("mutation", ["missing-manifest", "missing-lock", "alias", "path-escape", "path-mismatch",
                                     "metadata-path", "metadata-version", "new-target", "optional", "features",
                                     "external-source", "extra-manifest-change"])
def test_unmatched_or_broadened_manifest_edges_refuse(tmp_path, mutation):
    root = tmp_path.resolve()
    before, after, manifests, metadata = fixture(root)
    current = manifests["xtask/Cargo.toml"][1]
    dependency = metadata["packages"][0]["dependencies"][0]
    if mutation == "missing-manifest":
        del current["dependencies"]["kernel"]
    elif mutation == "missing-lock":
        after["package"][0]["dependencies"].remove("kernel")
    elif mutation == "alias":
        current["dependencies"]["kernel"]["package"] = "different-name"
    elif mutation == "path-escape":
        current["dependencies"]["kernel"]["path"] = "../../outside"
    elif mutation == "path-mismatch":
        current["dependencies"]["kernel"]["path"] = "../different"
    elif mutation == "metadata-path":
        dependency["path"] = str(root / "elsewhere")
    elif mutation == "metadata-version":
        metadata["packages"][1]["version"] = "1.0.1"
    elif mutation == "new-target":
        del manifests["crates/kernel/Cargo.toml"]
    elif mutation == "optional":
        dependency["optional"] = True
    elif mutation == "features":
        dependency["features"] = ["additional-feature"]
    elif mutation == "external-source":
        dependency["source"] = "registry+https://example.invalid/index"
    else:
        current["features"] = {"default": ["unexpected"]}
    with pytest.raises(guard.EdgeError):
        guard.validate(before, after, manifests, metadata, root)


def test_refuses_adding_dependency_to_an_external_package(tmp_path):
    root = tmp_path.resolve()
    before, after, manifests, metadata = fixture(root)
    after["package"][2]["dependencies"] = ["kernel"]
    with pytest.raises(guard.EdgeError, match="external package"):
        guard.validate(before, after, manifests, metadata, root)


def test_refuses_unqualified_name_shared_with_registry_package(tmp_path):
    root = tmp_path.resolve()
    before, after, manifests, metadata = fixture(root)
    registry = {"name": "kernel", "version": "2.0.0", "source": "registry+https://example.invalid/index", "checksum": "b" * 64}
    before["package"].append(registry)
    after["package"].append(copy.deepcopy(registry))
    with pytest.raises(guard.EdgeError, match="ambiguous"):
        guard.validate(before, after, manifests, metadata, root)


@pytest.mark.parametrize("reverse_kind", [None, "build", "dev"])
def test_new_normal_edge_cycle_uses_build_but_not_dev_dependencies(tmp_path, reverse_kind):
    root = tmp_path.resolve()
    before, after, manifests, metadata = fixture(root)
    for lock in (before, after):
        lock["package"][1]["dependencies"] = ["xtask"]
    table = "dependencies" if reverse_kind is None else reverse_kind + "-dependencies"
    for manifest in manifests["crates/kernel/Cargo.toml"]:
        manifest[table] = {"xtask": {"path": "../../xtask"}}
    metadata["packages"][1]["dependencies"] = [
        {"name": "xtask", "kind": reverse_kind, "path": str(root / "xtask")}]
    if reverse_kind == "dev":
        assert guard.validate(before, after, manifests, metadata, root) == {"xtask/Cargo.toml": ["kernel"]}
    else:
        with pytest.raises(guard.EdgeError, match="cycle"):
            guard.validate(before, after, manifests, metadata, root)


def relocate(root, values, package, directory):
    """Move one fixture package under `directory` in its manifests, metadata and edge path."""
    before, after, manifests, metadata = values
    old = {"xtask": "xtask", "kernel": "crates/kernel"}[package]
    new = f"{directory}/{package}"
    manifests[f"{new}/Cargo.toml"] = manifests.pop(f"{old}/Cargo.toml")
    entry = next(row for row in metadata["packages"] if row["name"] == package)
    entry["manifest_path"] = str(root / new / "Cargo.toml")
    source = "xtask" if package == "kernel" else new
    target = new if package == "kernel" else "crates/kernel"
    depth = len(Path(source).parts)
    manifests[f"{source}/Cargo.toml"][1]["dependencies"]["kernel"]["path"] = "/".join([".."] * depth + [target])
    metadata["packages"][0]["dependencies"][0]["path"] = str(root / target)
    return before, after, manifests, metadata


@pytest.mark.parametrize("package", ["kernel", "xtask"])
@pytest.mark.parametrize("directory,refused", [("vendor", True), ("patches", True), ("tools", False)])
def test_vendored_and_patched_members_are_never_local_edges(tmp_path, package, directory, refused):
    """Vendored workspace members are path packages but remain outside the exception."""
    root = tmp_path.resolve()
    values = relocate(root, fixture(root), package, directory)
    if refused:
        with pytest.raises(guard.EdgeError, match="vendored or patched"):
            guard.validate(*values, root)
    else:
        source = "xtask" if package == "kernel" else f"{directory}/xtask"
        assert guard.validate(*values, root) == {f"{source}/Cargo.toml": ["kernel"]}
