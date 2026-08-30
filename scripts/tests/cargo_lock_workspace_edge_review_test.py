#!/usr/bin/env python3
"""Authenticate reviewed Cargo.lock edges and the manifest feature change."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
import unittest
from collections import deque
from pathlib import Path
from typing import Any, Mapping


ROOT = Path(__file__).resolve().parents[2]
REVIEW_PATH = ROOT / "ci/cargo_lock_workspace_edge_review_v1.json"
REVIEW_SHA256 = "f8cefde6a33f86cedaa0b19ab04be7ecfbd851f009f792da2417f566a0d31da5"
BASE_COMMIT = "be874e0f08743929a492dd17383747a96a4f0879"
BASE_LOCK_BLOB = "5d04cef722cb695dd636110be01ff8de52ae7b45"
BASE_LOCK_SHA256 = "c90b3659d6cb44cd1d6f9e75e7b98aacc0d30bbe23041d4e6e109e8a206fa76b"
CANDIDATE_LOCK_BLOB = "dec1238701c58b8b5b906c26624865c685d5ac70"
CANDIDATE_LOCK_SHA256 = "179f589da420c024725efd9a65adb9c1e34085fa022cc01a8c67bb2262e93bf7"
EXPECTED_EDGE_KEYS = (
    ("crates/iroha/Cargo.toml", "dev-dependencies", "iroha_data_model"),
    ("crates/iroha_js_host/Cargo.toml", "dependencies", "iroha_torii_shared"),
    ("crates/iroha_zkp_halo2/Cargo.toml", "dependencies", "iroha_crypto"),
)
EXPECTED_SPEC_CHANGE_KEYS = (
    (
        "python/iroha_python/iroha_python_rs/Cargo.toml",
        "dependencies",
        "rustix",
    ),
)


class ReviewError(AssertionError):
    """The reviewed lock/manifest pair no longer matches the exact authority."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ReviewError(message)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _git(
    root: Path,
    *arguments: str,
    input_bytes: bytes | None = None,
) -> bytes:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=root,
            check=True,
            input=input_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode("utf-8", errors="replace").strip()
        raise ReviewError(f"git {' '.join(arguments)} failed: {detail}") from error


def _git_blob_oid(root: Path, data: bytes) -> str:
    return _git(root, "hash-object", "--stdin", input_bytes=data).decode().strip()


def _read_regular(path: Path, label: str) -> bytes:
    _require(path.is_file() and not path.is_symlink(), f"{label} must be a regular file")
    return path.read_bytes()


def _canonical_review(
    review_path: Path,
    review_bytes: bytes | None = None,
) -> tuple[dict[str, Any], bytes]:
    raw = review_bytes if review_bytes is not None else _read_regular(review_path, "review")
    _require(_sha256(raw) == REVIEW_SHA256, "review artifact digest changed")
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReviewError(f"review artifact is invalid JSON: {error}") from error
    canonical = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
    _require(raw == canonical, "review artifact must use canonical sorted JSON")
    _require(
        set(payload)
        == {
            "base",
            "candidate",
            "lock_semantic_delta",
            "permitted_edges",
            "permitted_spec_changes",
            "review_scope",
            "schema",
            "schema_version",
        },
        "review artifact top-level keys changed",
    )
    _require(
        payload.get("schema") == "iroha.cargo_lock_workspace_edge_review"
        and payload.get("schema_version") == 1
        and payload.get("review_scope")
        == "exact-base-current-lock-workspace-edges-and-manifest-specs",
        "review artifact schema changed",
    )
    _require(payload["base"]["commit"] == BASE_COMMIT, "reviewed base commit changed")
    _require(
        payload["base"]["cargo_lock"]["blob"] == BASE_LOCK_BLOB
        and payload["base"]["cargo_lock"]["sha256"] == BASE_LOCK_SHA256,
        "reviewed base lock authority changed",
    )
    _require(
        payload["candidate"]["cargo_lock"]["blob"] == CANDIDATE_LOCK_BLOB
        and payload["candidate"]["cargo_lock"]["sha256"]
        == CANDIDATE_LOCK_SHA256,
        "reviewed candidate lock authority changed",
    )
    return payload, raw


def _lock_metrics(document: Mapping[str, Any]) -> dict[str, int]:
    packages = document.get("package")
    _require(isinstance(packages, list), "Cargo.lock must contain package rows")
    return {
        "package_count": len(packages),
        "unique_package_name_count": len({row["name"] for row in packages}),
        "source_package_count": sum("source" in row for row in packages),
        "checksum_package_count": sum("checksum" in row for row in packages),
    }


def _authenticate_lock(
    root: Path,
    record: Mapping[str, Any],
    data: bytes,
    *,
    label: str,
) -> dict[str, Any]:
    _require(len(data) == record["bytes"], f"{label} byte count changed")
    _require(data.count(b"\n") == record["line_count"], f"{label} line count changed")
    _require(_sha256(data) == record["sha256"], f"{label} SHA-256 changed")
    _require(_git_blob_oid(root, data) == record["blob"], f"{label} git blob changed")
    try:
        document = tomllib.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ReviewError(f"{label} is invalid TOML: {error}") from error
    for key, value in _lock_metrics(document).items():
        _require(value == record[key], f"{label} {key} changed")
    return document


def _package_map(document: Mapping[str, Any]) -> dict[tuple[str, str, str | None], dict[str, Any]]:
    packages = document["package"]
    rows = {
        (row["name"], row["version"], row.get("source")): row
        for row in packages
    }
    _require(len(rows) == len(packages), "Cargo.lock contains duplicate package identities")
    return rows


def _lock_delta(
    base: Mapping[str, Any],
    candidate: Mapping[str, Any],
) -> tuple[list[dict[str, Any]], int, int, int]:
    base_packages = _package_map(base)
    candidate_packages = _package_map(candidate)
    added = set(candidate_packages) - set(base_packages)
    removed = set(base_packages) - set(candidate_packages)
    changed_metadata = 0
    changed_dependencies: list[dict[str, Any]] = []
    for identity in sorted(set(base_packages) & set(candidate_packages)):
        before = dict(base_packages[identity])
        after = dict(candidate_packages[identity])
        before_dependencies = before.pop("dependencies", [])
        after_dependencies = after.pop("dependencies", [])
        if before != after:
            changed_metadata += 1
        dependency_added = sorted(set(after_dependencies) - set(before_dependencies))
        dependency_removed = sorted(set(before_dependencies) - set(after_dependencies))
        if dependency_added or dependency_removed:
            changed_dependencies.append(
                {
                    "package": identity[0],
                    "version": identity[1],
                    "added": dependency_added,
                    "removed": dependency_removed,
                }
            )
    return changed_dependencies, len(added), len(removed), changed_metadata


def _dependency_tables(document: Mapping[str, Any]):
    for name in ("dependencies", "dev-dependencies", "build-dependencies"):
        table = document.get(name, {})
        _require(isinstance(table, dict), f"[{name}] must be a table")
        yield name, table
    workspace = document.get("workspace", {})
    if isinstance(workspace, dict):
        for name in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = workspace.get(name, {})
            _require(isinstance(table, dict), f"[workspace.{name}] must be a table")
            yield f"workspace.{name}", table
    targets = document.get("target", {})
    _require(isinstance(targets, dict), "[target] must be a table")
    for target_name in sorted(targets):
        target = targets[target_name]
        _require(isinstance(target, dict), f"[target.{target_name}] must be a table")
        for name in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = target.get(name, {})
            _require(isinstance(table, dict), f"[target.{target_name}.{name}] must be a table")
            yield f"target.{target_name}.{name}", table


def _normalized_spec(value: Any) -> dict[str, Any]:
    if isinstance(value, str):
        return {"version": value}
    _require(isinstance(value, dict), "dependency specification must be a string or table")
    return dict(value)


def _dependency_map(text: bytes, label: str) -> dict[tuple[str, str], dict[str, Any]]:
    try:
        document = tomllib.loads(text.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ReviewError(f"{label} is invalid TOML: {error}") from error
    return {
        (section, alias): _normalized_spec(spec)
        for section, table in _dependency_tables(document)
        for alias, spec in table.items()
    }


def _manifest_paths(root: Path, base_commit: str) -> tuple[str, ...]:
    current = {
        row.decode()
        for row in _git(root, "ls-files", "-z", "*Cargo.toml").split(b"\0")
        if row
    }
    base = {
        row.decode()
        for row in _git(
            root,
            "ls-tree",
            "-r",
            "--name-only",
            "-z",
            base_commit,
        ).split(b"\0")
        if row and row.decode().endswith("Cargo.toml")
    }
    _require(current == base, "Cargo manifest path inventory changed")
    return tuple(sorted(current))


def _manifest_delta(
    root: Path,
    base_commit: str,
    overrides: Mapping[str, bytes],
) -> tuple[
    dict[tuple[str, str, str], dict[str, Any]],
    dict[tuple[str, str, str], dict[str, Any]],
    dict[tuple[str, str, str], tuple[dict[str, Any], dict[str, Any]]],
]:
    base_specs: dict[tuple[str, str, str], dict[str, Any]] = {}
    current_specs: dict[tuple[str, str, str], dict[str, Any]] = {}
    for path in _manifest_paths(root, base_commit):
        before = _git(root, "show", f"{base_commit}:{path}")
        after = overrides.get(path, (root / path).read_bytes())
        for (section, alias), spec in _dependency_map(before, f"base {path}").items():
            base_specs[(path, section, alias)] = spec
        for (section, alias), spec in _dependency_map(after, f"current {path}").items():
            current_specs[(path, section, alias)] = spec
    added = {key: current_specs[key] for key in set(current_specs) - set(base_specs)}
    removed = {key: base_specs[key] for key in set(base_specs) - set(current_specs)}
    changed = {
        key: (base_specs[key], current_specs[key])
        for key in set(base_specs) & set(current_specs)
        if base_specs[key] != current_specs[key]
    }
    return added, removed, changed


def _current_documents(
    root: Path,
    base_commit: str,
    overrides: Mapping[str, bytes],
) -> dict[str, dict[str, Any]]:
    documents = {}
    for path in _manifest_paths(root, base_commit):
        raw = overrides.get(path, (root / path).read_bytes())
        documents[path] = tomllib.loads(raw.decode())
    return documents


def _manifest_path(root: Path, owner: str, raw_path: str, inherited: bool) -> Path:
    path = Path(raw_path)
    if not path.is_absolute():
        path = (root if inherited else (root / owner).parent) / path
    if path.name != "Cargo.toml":
        path = path / "Cargo.toml"
    return path.resolve()


def _local_dependency_graph(
    root: Path,
    documents: Mapping[str, Mapping[str, Any]],
) -> tuple[dict[str, set[str]], dict[str, str]]:
    root_document = documents["Cargo.toml"]
    workspace = root_document.get("workspace", {})
    _require(isinstance(workspace, dict), "root manifest lost [workspace]")
    workspace_dependencies = workspace.get("dependencies", {})
    _require(isinstance(workspace_dependencies, dict), "root manifest lost workspace dependencies")
    resolved_paths = {(root / path).resolve(): path for path in documents}
    patches: dict[str, str] = {}
    patch_tables = root_document.get("patch", {})
    _require(isinstance(patch_tables, dict), "root manifest [patch] must be a table")
    for registry, table in patch_tables.items():
        _require(isinstance(table, dict), f"root manifest [patch.{registry}] must be a table")
        for alias, raw_spec in table.items():
            spec = _normalized_spec(raw_spec)
            raw_path = spec.get("path")
            if not isinstance(raw_path, str):
                continue
            target = resolved_paths.get(_manifest_path(root, "Cargo.toml", raw_path, True))
            if target is not None:
                package_name = spec.get("package", alias)
                _require(isinstance(package_name, str), f"patch {alias} package changed")
                patches[package_name] = target
    package_paths: dict[str, str] = {}
    for path, document in documents.items():
        package = document.get("package")
        if isinstance(package, dict) and isinstance(package.get("name"), str):
            _require(package["name"] not in package_paths, f"duplicate local package {package['name']}")
            package_paths[package["name"]] = path

    graph = {path: set() for path in package_paths.values()}
    for owner, document in documents.items():
        package = document.get("package")
        if not isinstance(package, dict) or owner not in graph:
            continue
        for _section, table in _dependency_tables(document):
            for alias, raw_spec in table.items():
                member_spec = _normalized_spec(raw_spec)
                inherited = member_spec.get("workspace") is True
                if inherited:
                    _require(alias in workspace_dependencies, f"unknown workspace dependency {alias}")
                    spec = _normalized_spec(workspace_dependencies[alias])
                    spec.update({key: value for key, value in member_spec.items() if key != "workspace"})
                else:
                    spec = member_spec
                raw_path = spec.get("path")
                package_name = spec.get("package", alias)
                _require(isinstance(package_name, str), f"dependency {alias} package changed")
                target = (
                    resolved_paths.get(_manifest_path(root, owner, raw_path, inherited))
                    if isinstance(raw_path, str)
                    else patches.get(package_name)
                )
                if target is not None:
                    graph[owner].add(target)
    return graph, package_paths


def _reachable_count_and_cycle(
    graph: Mapping[str, set[str]],
    dependency: str,
    owner: str,
) -> tuple[int, bool]:
    visited = {dependency}
    queue = deque([dependency])
    while queue:
        node = queue.popleft()
        for target in sorted(graph.get(node, ())):
            if target not in visited:
                visited.add(target)
                queue.append(target)
    return len(visited), owner in visited


def _validate_edges(
    root: Path,
    base_commit: str,
    payload: Mapping[str, Any],
    overrides: Mapping[str, bytes],
) -> None:
    added, removed, changed = _manifest_delta(root, base_commit, overrides)
    _require(not removed, f"dependency edges were removed: {sorted(removed)}")
    _require(
        set(changed) == set(EXPECTED_SPEC_CHANGE_KEYS),
        f"unreviewed dependency specification changes: {sorted(changed)}",
    )
    _require(set(added) == set(EXPECTED_EDGE_KEYS), f"unreviewed dependency additions: {sorted(added)}")

    edges = payload["permitted_edges"]
    _require(isinstance(edges, list) and len(edges) == 3, "review must contain exactly three edges")
    spec_changes = payload["permitted_spec_changes"]
    _require(
        isinstance(spec_changes, list) and len(spec_changes) == 1,
        "review must contain exactly one manifest specification change",
    )
    reviewed_spec = spec_changes[0]
    reviewed_spec_key = (
        reviewed_spec["manifest"],
        reviewed_spec["manifest_section"],
        reviewed_spec["dependency"],
    )
    _require(
        reviewed_spec_key in EXPECTED_SPEC_CHANGE_KEYS,
        f"unexpected reviewed specification change {reviewed_spec_key}",
    )
    before_spec, after_spec = changed[reviewed_spec_key]
    _require(
        before_spec == reviewed_spec["before_spec"]
        and after_spec == reviewed_spec["after_spec"],
        f"{reviewed_spec_key} specification authority changed",
    )
    _require(
        reviewed_spec["lock_semantic_effect"] == "none"
        and before_spec == {"version": "1.1.4", "features": ["process"]}
        and after_spec
        == {"version": "1.1.4", "features": ["fs", "process"]},
        f"{reviewed_spec_key} must add only the reviewed rustix fs feature",
    )
    documents = _current_documents(root, base_commit, overrides)
    graph, package_paths = _local_dependency_graph(root, documents)
    root_workspace = documents["Cargo.toml"]["workspace"]["dependencies"]
    for edge in edges:
        key = (edge["manifest"], edge["manifest_section"], edge["dependency"])
        _require(key in EXPECTED_EDGE_KEYS, f"unexpected reviewed edge {key}")
        _require(added[key] == edge["member_spec"], f"{key} member specification changed")
        _require(
            _normalized_spec(root_workspace[edge["dependency"]]) == edge["workspace_spec"],
            f"{key} workspace specification changed",
        )
        _require(package_paths[edge["owner"]] == edge["manifest"], f"{key} owner changed")
        _require(
            package_paths[edge["dependency"]] == edge["target_manifest"],
            f"{key} target changed",
        )
        target_features = documents[edge["target_manifest"]].get("features", {})
        target_defaults = target_features.get("default", []) if isinstance(target_features, dict) else []
        _require(target_defaults == edge["target_default_features"], f"{key} target defaults changed")
        merged = dict(edge["workspace_spec"])
        merged.update(
            {name: value for name, value in edge["member_spec"].items() if name != "workspace"}
        )
        default_features = merged.get("default-features", True)
        requested_features = merged.get("features", [])
        _require(default_features is edge["effective_default_features"], f"{key} default feature policy changed")
        _require(requested_features == edge["requested_features"], f"{key} requested features changed")
        effective = list(requested_features)
        if default_features:
            effective.extend(target_defaults)
        _require(effective == edge["effective_direct_features"], f"{key} widens features")
        reachable, cyclic = _reachable_count_and_cycle(
            graph,
            edge["target_manifest"],
            edge["manifest"],
        )
        _require(not cyclic, f"{key} closes a local dependency cycle")
        _require(
            reachable == edge["reachable_from_dependency_without_edge"],
            f"{key} local reachability changed",
        )


def validate_review(
    root: Path,
    base_commit: str,
    review_path: Path,
    *,
    review_bytes: bytes | None = None,
    candidate_lock_bytes: bytes | None = None,
    manifest_overrides: Mapping[str, bytes] | None = None,
) -> None:
    root = root.resolve()
    payload, _raw = _canonical_review(review_path, review_bytes)
    resolved_base = _git(root, "rev-parse", f"{base_commit}^{{commit}}").decode().strip()
    _require(resolved_base == BASE_COMMIT, "AGENTS base is not the reviewed commit")
    _require(
        _git(root, "rev-parse", f"{resolved_base}:Cargo.lock").decode().strip()
        == BASE_LOCK_BLOB,
        "base commit does not contain the reviewed Cargo.lock blob",
    )
    base_lock_bytes = _git(root, "show", f"{resolved_base}:Cargo.lock")
    current_lock_bytes = (
        candidate_lock_bytes
        if candidate_lock_bytes is not None
        else _read_regular(root / "Cargo.lock", "candidate Cargo.lock")
    )
    base_lock = _authenticate_lock(
        root,
        payload["base"]["cargo_lock"],
        base_lock_bytes,
        label="base Cargo.lock",
    )
    candidate_lock = _authenticate_lock(
        root,
        payload["candidate"]["cargo_lock"],
        current_lock_bytes,
        label="candidate Cargo.lock",
    )
    changed, added, removed, changed_metadata = _lock_delta(base_lock, candidate_lock)
    expected_delta = payload["lock_semantic_delta"]
    _require(changed == expected_delta["changed_dependency_lists"], "Cargo.lock dependency delta changed")
    _require(added == expected_delta["added_package_identities"], "Cargo.lock added packages")
    _require(removed == expected_delta["removed_package_identities"], "Cargo.lock removed packages")
    _require(changed_metadata == expected_delta["changed_package_metadata"], "Cargo.lock package metadata changed")
    _validate_edges(root, resolved_base, payload, manifest_overrides or {})


def _validate_agents_guard_source(source: str) -> None:
    frozen_assignment = f'readonly lock_review_base_commit="{BASE_COMMIT}"'
    _require(
        source.count(frozen_assignment) == 1,
        "AGENTS guard must bind exactly one frozen lock-review base",
    )
    _require(
        'git diff --quiet "${lock_review_base_commit}" -- Cargo.lock' in source,
        "AGENTS guard lock comparison must use the frozen review base",
    )
    _require(
        '--base-commit "${lock_review_base_commit}"' in source,
        "AGENTS guard validator invocation must use the frozen review base",
    )
    _require(
        '--base-commit "${base_commit}"' not in source,
        "AGENTS guard must not derive lock-review authority from the ambient base",
    )
    _require(
        'dependency_base_commit="${base_commit}"' in source
        and 'dependency_base_commit="${lock_review_base_commit}"' in source,
        "AGENTS guard must switch dependency review to the frozen lock-review base",
    )
    _require(
        'git diff --name-status --diff-filter=A "${dependency_base_commit}"...HEAD'
        in source
        and 'git show "${dependency_base_commit}:Cargo.toml"' in source
        and 'BASE_COMMIT="${dependency_base_commit}" python3' in source,
        "AGENTS guard dependency checks must consume the selected review base",
    )
    _require(
        'BASE_COMMIT="${base_commit}" python3' not in source,
        "AGENTS guard dependency scanner must not bypass the selected review base",
    )


class CargoLockWorkspaceEdgeReviewTest(unittest.TestCase):
    def test_current_review_is_exact(self) -> None:
        validate_review(ROOT, BASE_COMMIT, REVIEW_PATH)

    def test_review_and_candidate_lock_mutations_fail_closed(self) -> None:
        raw = REVIEW_PATH.read_bytes()
        payload = json.loads(raw)
        payload["candidate"]["cargo_lock"]["sha256"] = "0" * 64
        mutated_review = (json.dumps(payload, indent=2, sort_keys=True) + "\n").encode()
        with self.assertRaisesRegex(ReviewError, "review artifact digest"):
            validate_review(ROOT, BASE_COMMIT, REVIEW_PATH, review_bytes=mutated_review)
        with self.assertRaisesRegex(ReviewError, "candidate Cargo.lock"):
            validate_review(
                ROOT,
                BASE_COMMIT,
                REVIEW_PATH,
                candidate_lock_bytes=(ROOT / "Cargo.lock").read_bytes() + b"\n",
            )

    def test_unreviewed_manifest_edge_fails_closed(self) -> None:
        path = "crates/iroha_js_host/Cargo.toml"
        source = (ROOT / path).read_bytes()
        mutated = source.replace(
            b"iroha_torii_shared = { workspace = true }\n",
            b"iroha_torii_shared = { workspace = true }\nserde = { workspace = true }\n",
            1,
        )
        with self.assertRaisesRegex(ReviewError, "unreviewed dependency additions"):
            validate_review(
                ROOT,
                BASE_COMMIT,
                REVIEW_PATH,
                manifest_overrides={path: mutated},
            )

    def test_cycle_detector_rejects_a_back_edge(self) -> None:
        graph = {"owner": {"dependency"}, "dependency": {"transitive"}, "transitive": set()}
        self.assertEqual(_reachable_count_and_cycle(graph, "dependency", "owner"), (2, False))
        graph["transitive"].add("owner")
        self.assertEqual(_reachable_count_and_cycle(graph, "dependency", "owner"), (3, True))

    def test_agents_guard_consumes_only_the_exact_review(self) -> None:
        guard = (ROOT / "ci/check_agents_guardrails.sh").read_text()
        _validate_agents_guard_source(guard)
        self.assertEqual(guard.count("cargo_lock_workspace_edge_review_test.py"), 1)
        self.assertEqual(guard.count("cargo_lock_workspace_edge_review_v1.json"), 1)
        self.assertIn('reviewed_workspace_edge_change="false"', guard)
        self.assertNotIn("DEPENDENCY_DISCIPLINE_ALLOW", guard)

    def test_agents_guard_rejects_ambient_lock_review_base_mutations(self) -> None:
        guard = (ROOT / "ci/check_agents_guardrails.sh").read_text()
        mutated_assignment = guard.replace(
            f'readonly lock_review_base_commit="{BASE_COMMIT}"',
            'readonly lock_review_base_commit="${base_commit}"',
            1,
        )
        with self.assertRaisesRegex(ReviewError, "frozen lock-review base"):
            _validate_agents_guard_source(mutated_assignment)

        mutated_comparison = guard.replace(
            'git diff --quiet "${lock_review_base_commit}" -- Cargo.lock',
            'git diff --quiet "${base_commit}"...HEAD -- Cargo.lock',
            1,
        )
        with self.assertRaisesRegex(ReviewError, "lock comparison"):
            _validate_agents_guard_source(mutated_comparison)

        mutated_invocation = guard.replace(
            '--base-commit "${lock_review_base_commit}"',
            '--base-commit "${base_commit}"',
            1,
        )
        with self.assertRaisesRegex(ReviewError, "validator invocation"):
            _validate_agents_guard_source(mutated_invocation)

        mutated_dependency_base = guard.replace(
            'dependency_base_commit="${lock_review_base_commit}"',
            'dependency_base_commit="${base_commit}"',
            1,
        )
        with self.assertRaisesRegex(ReviewError, "switch dependency review"):
            _validate_agents_guard_source(mutated_dependency_base)

    def test_privacy_sdk_guard_authenticates_the_frozen_review(self) -> None:
        workflow = (ROOT / ".github/workflows/pr_privacy_sdk_guard.yml").read_text()
        for path in (
            "ci/check_agents_guardrails.sh",
            "ci/cargo_lock_workspace_edge_review_v1.json",
            "scripts/tests/cargo_lock_workspace_edge_review_test.py",
        ):
            self.assertIn(f'      - "{path}"', workflow)

        privacy_guard = workflow.split("\n  privacy-sdk-guard:\n", 1)[1]
        self.assertIn(
            "      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262\n"
            "        with:\n"
            "          fetch-depth: 0",
            privacy_guard,
        )
        self.assertEqual(
            privacy_guard.count(
                "Authenticate frozen-base Cargo lock workspace-edge review"
            ),
            1,
        )
        self.assertIn(
            "scripts/tests/cargo_lock_workspace_edge_review_test.py \\\n"
            "            --validate \\\n"
            "            --root \"$GITHUB_WORKSPACE\" \\\n"
            f"            --base-commit {BASE_COMMIT} \\\n"
            "            --review \"$GITHUB_WORKSPACE/ci/"
            "cargo_lock_workspace_edge_review_v1.json\"",
            privacy_guard,
        )


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate", action="store_true")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--base-commit", default=BASE_COMMIT)
    parser.add_argument("--review", type=Path, default=REVIEW_PATH)
    return parser.parse_args()


if __name__ == "__main__":
    arguments = _parse_args()
    if not arguments.validate:
        raise SystemExit("--validate is required outside the unit-test runner")
    try:
        validate_review(
            arguments.root,
            arguments.base_commit,
            arguments.review,
        )
    except ReviewError as error:
        print(f"reviewed Cargo.lock dependency change rejected: {error}", file=sys.stderr)
        raise SystemExit(1) from error
    print(
        "reviewed Cargo.lock dependency change passed: "
        f"{BASE_LOCK_SHA256} -> {CANDIDATE_LOCK_SHA256}; "
        "exact three local edges plus the rustix fs feature"
    )
