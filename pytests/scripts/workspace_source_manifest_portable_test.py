"""Native clean-index manifest controls; no Unix physical source-seal claim.

Run on Windows, Linux and macOS with pytest and Git. Windows exercises its real
host entry point; Unix additionally exercises Windows checkout materialization.
The unchanged descriptor-seal suite remains mandatory on both Unix CI hosts.
"""
from __future__ import annotations

import hashlib
import importlib.util
import os
from pathlib import Path
import struct
import subprocess
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/compute_workspace_source_manifest.py"


def git(root, *args, data=None):
    return subprocess.run(["git", *args], cwd=root, input=data, check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.strip()


def fixture_commit(root):
    """Create synthetic Git data without invoking a signer or repository hooks."""
    tree = git(root, "write-tree").decode()
    body = (f"tree {tree}\nauthor Fixture <fixture@example.invalid> 1 +0000\n"
            "committer Fixture <fixture@example.invalid> 1 +0000\n\nfixture\n").encode()
    oid = git(root, "hash-object", "-t", "commit", "-w", "--stdin", data=body)
    git(root, "update-ref", "HEAD", oid.decode())
    return oid.decode()


@pytest.fixture
def checkout(tmp_path, monkeypatch):
    spec = importlib.util.spec_from_file_location("portable_manifest", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if os.name == "nt":
        assert not module._descriptor_source_manifest_supported()
        assert module._windows_host_semantics()
    else:
        monkeypatch.setattr(module, "_descriptor_source_manifest_supported", lambda: False)
        monkeypatch.setattr(module, "_windows_host_semantics", lambda: True)
    git(tmp_path, "init", "-q")
    for name, value in (("core.autocrlf", "false"), ("core.filemode", "false"),
                        ("core.symlinks", "false")):
        git(tmp_path, "config", name, value)
    files = {".gitignore": b"target/\n", "Cargo.lock": b"version = 3\n",
             "tracked.txt": b"source\n", "runner.sh": b"#!/bin/sh\nexit 0\n",
             "source-link": b"tracked.txt", "tree/payload": b"nested source\n"}
    for name, payload in files.items():
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
    git(tmp_path, "add", ".")
    for mode, name in (("100755", "runner.sh"), ("120000", "source-link")):
        oid = git(tmp_path, "hash-object", "-w", "--stdin", data=files[name]).decode()
        git(tmp_path, "update-index", "--cacheinfo", mode, oid, name)
    nested_oid = fixture_commit(tmp_path)
    git(tmp_path, "update-index", "--add", "--cacheinfo", "160000", nested_oid, "nested")
    (tmp_path / "nested").mkdir()
    fixture_commit(tmp_path)
    return module, tmp_path, files


def expected_manifest(files):
    """Independent public framing oracle using literal fixture bytes/index modes."""
    digest = hashlib.sha256(b"iroha-workspace-source-manifest-v2\0")
    for name in sorted([*files, "nested"]):
        raw = name.encode()
        digest.update(struct.pack(">Q", len(raw)) + raw)
        if name == "nested":
            digest.update(struct.pack(">I", 0o755) + b"G")
        elif name == "source-link":
            target = files[name]
            digest.update(struct.pack(">I", 0o777) + b"L" + struct.pack(">Q", len(target)) + target)
        else:
            payload = files[name]
            mode = 0o755 if name == "runner.sh" else 0o644
            digest.update(struct.pack(">I", mode) + b"F" + struct.pack(">Q", len(payload)) + payload)
    return digest.hexdigest()


def test_native_entrypoint_authenticates_canonical_index_bytes_and_modes(checkout):
    module, root, files = checkout
    assert module.native_artifact_workspace_source_manifest(root) == expected_manifest(files)
    # Local Windows installation config is not an authority over staged modes.
    git(root, "config", "core.filemode", "true")
    git(root, "config", "--unset-all", "core.symlinks")
    assert module.native_artifact_workspace_source_manifest(root) == expected_manifest(files)


@pytest.mark.parametrize("name", ["tracked.txt", "Cargo.lock", "source-link", "tree/payload"])
def test_native_entrypoint_rejects_raw_byte_drift(checkout, name):
    module, root, _ = checkout
    (root / name).write_bytes(b"tampered\n")
    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.native_artifact_workspace_source_manifest(root)


def test_native_entrypoint_rejects_staged_changes(checkout):
    module, root, _ = checkout
    (root / "tracked.txt").write_bytes(b"staged\n")
    git(root, "add", "tracked.txt")
    with pytest.raises(module.DirtyReleaseSourceError, match="index is not HEAD"):
        module.native_artifact_workspace_source_manifest(root)


@pytest.mark.parametrize("local_exclude", [False, True])
def test_native_entrypoint_rejects_untracked_source_even_with_local_exclude(checkout, local_exclude):
    module, root, _ = checkout
    (root / "injected.rs").write_bytes(b"unsealed\n")
    if local_exclude:
        (root / ".git/info/exclude").write_bytes(b"injected.rs\n")
    with pytest.raises(module.DirtyReleaseSourceError, match="stage-zero index"):
        module.native_artifact_workspace_source_manifest(root)


def test_native_entrypoint_rejects_untracked_ignore_policy(checkout):
    module, root, _ = checkout
    (root / "tree/.gitignore").write_bytes(b"injected.rs\n")
    (root / "tree/injected.rs").write_bytes(b"unsealed\n")
    with pytest.raises(module.DirtyReleaseSourceError, match="untracked ignore policy"):
        module.native_artifact_workspace_source_manifest(root)


def test_native_entrypoint_keeps_tracked_input_below_ignored_output(checkout):
    module, root, files = checkout
    (root / "target").mkdir()
    (root / "target/forced.txt").write_bytes(b"tracked\n")
    git(root, "add", "-f", "target/forced.txt")
    fixture_commit(root)
    files["target/forced.txt"] = b"tracked\n"
    assert module.native_artifact_workspace_source_manifest(root) == expected_manifest(files)
    (root / "target/forced.txt").write_bytes(b"tampered\n")
    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.native_artifact_workspace_source_manifest(root)


def test_native_entrypoint_rejects_populated_gitlink(checkout):
    module, root, _ = checkout
    (root / "nested/injected").write_bytes(b"unsealed\n")
    with pytest.raises(module.DirtyReleaseSourceError, match="gitlink must be one empty"):
        module.native_artifact_workspace_source_manifest(root)


@pytest.mark.parametrize("member", ["tracked.txt", "tree", "source-link", "nested"])
def test_portable_snapshot_rejects_reparse_members_and_parents(checkout, monkeypatch, member):
    module, root, _ = checkout
    entries = module._git_index_entries(root)
    original = Path.lstat
    def reparse(path):
        metadata = original(path)
        if path == root / member:
            fields = {name: getattr(metadata, name) for name in dir(metadata) if name.startswith("st_")}
            fields["st_reparse_tag"] = 0xA0000003
            return SimpleNamespace(**fields)
        return metadata
    monkeypatch.setattr(Path, "lstat", reparse)
    with pytest.raises((module.SourceSealError, module.DirtyReleaseSourceError)):
        module._portable_clean_index_manifest_snapshot(root, entries)


def test_portable_reader_rejects_same_byte_file_replacement(checkout, monkeypatch):
    module, root, files = checkout
    path = root / "tracked.txt"
    replacement = root / ".git/replacement"
    replacement.write_bytes(files["tracked.txt"])
    original = module.os.open
    def replace_before_open(candidate, flags, *args, **kwargs):
        if Path(candidate) == path:
            path.replace(root / ".git/retained-original")
            replacement.replace(path)
        return original(candidate, flags, *args, **kwargs)
    monkeypatch.setattr(module.os, "open", replace_before_open)
    with pytest.raises(module.SourceSealError, match="changed before it was opened"):
        module.native_artifact_workspace_source_manifest(root)


def test_portable_snapshot_rejects_parent_replacement(checkout, monkeypatch):
    module, root, _ = checkout
    entries = module._git_index_entries(root)
    original = module._require_same_portable_source_parents
    def replace_parent(parents):
        parents = list(parents)
        if any(path == root / "tree" for path, _ in parents):
            (root / "tree").rename(root / ".git/retained-tree")
            (root / "tree").mkdir()
        return original(parents)
    monkeypatch.setattr(module, "_require_same_portable_source_parents", replace_parent)
    with pytest.raises(module.SourceSealError, match="parent changed"):
        module._portable_clean_index_manifest_snapshot(root, entries)


@pytest.mark.parametrize("change", ["head", "index", "membership"])
def test_native_entrypoint_rechecks_capture_boundaries(checkout, monkeypatch, change):
    module, root, _ = checkout
    original = module._portable_clean_index_manifest_snapshot
    def changed_snapshot(*args):
        result = original(*args)
        if change == "head":
            tree = git(root, "write-tree").decode()
            body = (f"tree {tree}\nauthor Fixture <f@example.invalid> 2 +0000\n"
                    "committer Fixture <f@example.invalid> 2 +0000\n\nnew\n").encode()
            oid = git(root, "hash-object", "-t", "commit", "-w", "--stdin", data=body)
            git(root, "update-ref", "HEAD", oid.decode())
        elif change == "index":
            (root / "tracked.txt").write_bytes(b"new staged bytes\n")
            git(root, "add", "tracked.txt")
        else:
            (root / "injected.rs").write_bytes(b"new member\n")
        return result
    monkeypatch.setattr(module, "_portable_clean_index_manifest_snapshot", changed_snapshot)
    diagnostic = {"head": "HEAD changed", "index": "index changed", "membership": "paths changed"}[change]
    with pytest.raises(module.DirtyReleaseSourceError, match=diagnostic):
        module.native_artifact_workspace_source_manifest(root)


def test_native_entrypoint_rejects_active_operation(checkout):
    module, root, _ = checkout
    (root / ".git/MERGE_HEAD").write_bytes(b"1" * 40 + b"\n")
    with pytest.raises(module.ActiveGitOperationError):
        module.native_artifact_workspace_source_manifest(root)


def test_portable_snapshot_rejects_actual_symlink_parent(checkout):
    module, root, _ = checkout
    entries = module._git_index_entries(root)
    (root / "tree").rename(root / ".git/retained-tree")
    (root / "tree").symlink_to(root / ".git/retained-tree", target_is_directory=True)
    with pytest.raises(module.SourceSealError, match="symlink.*parent"):
        module._portable_clean_index_manifest_snapshot(root, entries)


def _require_platform_coverage(workflow):
    step = workflow.split("      - name: Exercise retained Python source delivery\n", 1)[1].split("      - name:", 1)[0]
    selection = '''          source_manifest_tests=(pytests/scripts/workspace_source_manifest_portable_test.py)
          if [[ "${RUNNER_OS}" == "Windows" ]]; then
            source_manifest_tests+=(
              pytests/scripts/workspace_source_manifest_test.py::test_windows_reader_normalizes_only_cross_api_ctime_and_execute_bits
              pytests/scripts/workspace_source_manifest_test.py::test_native_artifact_manifest_cli_uses_host_portable_entrypoint
              pytests/scripts/workspace_source_manifest_test.py::test_source_path_list_rejects_unsafe_archive_names
              pytests/scripts/workspace_source_manifest_test.py::test_source_seal_rejects_non_posix_symlink_targets
              pytests/scripts/workspace_source_manifest_test.py::test_git_unmerged_paths_are_parsed_and_deduplicated
              pytests/scripts/workspace_source_manifest_test.py::test_workspace_manifest_rejects_unmerged_index
              pytests/scripts/workspace_source_manifest_test.py::test_workspace_manifest_rejects_active_git_operations
              pytests/scripts/workspace_source_manifest_test.py::test_workspace_manifest_rejects_resolved_but_uncommitted_merge
              pytests/scripts/workspace_source_manifest_test.py::test_release_identity_rejects_staged_source_without_git_mutation
              pytests/scripts/workspace_source_manifest_test.py::test_release_identity_rejects_unmerged_before_deriving_a_tree
              pytests/scripts/workspace_source_manifest_test.py::test_manifest_rejects_effective_self_hiding_ignore_policy
              pytests/scripts/workspace_source_manifest_test.py::test_manifest_local_exclude_cannot_authorize_untracked_policy
              pytests/scripts/workspace_source_manifest_test.py::test_manifest_untracked_policy_cannot_authorize_nested_policy
              pytests/scripts/workspace_source_manifest_test.py::test_release_identity_does_not_ignore_staged_gitlink_change
              pytests/scripts/workspace_source_manifest_test.py::test_ignore_policy_discovery_without_descriptors_keeps_source_and_policy_guards
              pytests/scripts/workspace_source_manifest_test.py::test_ignore_policy_discovery_does_not_prune_root_policy_from_local_excludes
              pytests/scripts/workspace_source_manifest_test.py::test_ignore_policy_discovery_does_not_prune_reopened_root_directory
              pytests/scripts/workspace_source_manifest_test.py::test_ignore_policy_discovery_does_not_trust_untracked_root_policy
              pytests/scripts/workspace_source_manifest_test.py::test_retained_scanner_success_does_not_forward_ambient_exception
              pytests/scripts/workspace_source_manifest_test.py::test_retained_scanner_body_failure_keeps_first_exception_and_refuses
              pytests/scripts/workspace_source_manifest_test.py::test_retained_scanner_metadata_failure_closes_and_refuses
              pytests/scripts/workspace_source_manifest_test.py::test_retained_scanner_actual_read_failure_closes_and_refuses
            )
          else
            source_manifest_tests+=(pytests/scripts/workspace_source_manifest_test.py)
          fi
'''
    assert step.count(selection) == 1
    assert step.count('            "${source_manifest_tests[@]}" \\\n') == 1
    assert step.count("          python -m pytest -q \\\n") == 1
    assert " -k " not in step and " -m " not in step.replace("python -m pytest", "pytest").replace("python -m pip", "pip")
    for name in ("python_native_source_delivery_test", "python_native_delivery_gate_contract_test",
                 "python_wheel_byte_owner_test", "python_source_owner_admission_test",
                 "sorafs_python_native_platform_test", "workspace_source_keeper_test"):
        assert step.count(name + ".py") == 1
    assert '      - "pytests/scripts/workspace_source_manifest_portable_test.py"\n' in workflow


def test_platform_workflow_preserves_unix_suite_and_windows_controls():
    workflow = (ROOT / ".github/workflows/platform_native.yml").read_text()
    _require_platform_coverage(workflow)
    for original, changed in (
        ('"${RUNNER_OS}" == "Windows"', '"${RUNNER_OS}" != "Windows"'),
        ('source_manifest_tests+=(pytests/scripts/workspace_source_manifest_test.py)', ': omitted'),
        ('              pytests/scripts/workspace_source_manifest_test.py::test_windows_reader_normalizes_only_cross_api_ctime_and_execute_bits\n', ''),
        ('            )\n          else\n', '              pytests/scripts/workspace_source_manifest_test.py::test_unreviewed_case\n            )\n          else\n'),
        ('source_manifest_tests=(pytests/scripts/workspace_source_manifest_portable_test.py)', 'source_manifest_tests=()'),
        ('python -m pytest -q \\', 'python -m pytest -q -k portable \\'),
        ('            scripts/tests/python_source_owner_admission_test.py \\\n', ''),
    ):
        assert original in workflow
        with pytest.raises(AssertionError):
            _require_platform_coverage(workflow.replace(original, changed, 1))


def test_native_entrypoint_never_executes_repository_clean_filter(checkout):
    module, root, files = checkout
    attributes = b"tracked.txt filter=forbidden\n"
    (root / ".gitattributes").write_bytes(attributes)
    git(root, "add", ".gitattributes")
    fixture_commit(root)
    files[".gitattributes"] = attributes
    git(root, "config", "filter.forbidden.clean", "iroha-test-forbidden-clean-filter")
    git(root, "config", "filter.forbidden.required", "true")
    assert module.native_artifact_workspace_source_manifest(root) == expected_manifest(files)
    (root / "tracked.txt").write_bytes(b"tampered\n")
    with pytest.raises(module.DirtyReleaseSourceError, match="tracked changes"):
        module.native_artifact_workspace_source_manifest(root)


def test_native_manifest_cli_uses_the_actual_host_entrypoint(checkout, monkeypatch, capsys):
    module, root, files = checkout
    monkeypatch.setattr(module.sys, "argv", [str(SCRIPT), "--root", str(root), "--native-artifact-manifest"])
    assert module.main() == 0
    assert capsys.readouterr().out.strip() == expected_manifest(files)


def test_portable_path_list_refuses_replacement_after_read(tmp_path, monkeypatch):
    spec = importlib.util.spec_from_file_location("portable_path_list", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    path = tmp_path / "source-paths.bin"
    replacement = tmp_path / "replacement.bin"
    retained = tmp_path / "retained.bin"
    module.write_source_path_list(path, ["a.txt"])
    module.write_source_path_list(replacement, ["b.txt"])
    original = Path.lstat
    observations = 0
    def replace_after_read(candidate):
        nonlocal observations
        if candidate == path:
            observations += 1
            if observations == 2:
                path.rename(retained)
                replacement.rename(path)
        return original(candidate)
    monkeypatch.setattr(Path, "lstat", replace_after_read)
    # Windows refuses the rename while the original reader is retained; Unix
    # permits it and the named-identity recheck detects the replacement.
    diagnostic = "changed after it was read" if os.name == "nt" else "replaced while it was read"
    with pytest.raises(module.SourcePathListError, match=diagnostic):
        module.read_source_path_list(path)


def test_portable_path_list_roundtrip_and_refusal_controls(tmp_path):
    spec = importlib.util.spec_from_file_location("portable_path_list_controls", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    path = tmp_path / "source-paths.bin"
    module.write_source_path_list(path, ["b.txt", "a.txt"])
    assert module.read_source_path_list(path) == ["a.txt", "b.txt"]
    malformed = tmp_path / "malformed.bin"
    malformed.write_bytes(path.read_bytes() + b"trailing")
    with pytest.raises(module.SourcePathListError, match="trailing bytes"):
        module.read_source_path_list(malformed)
    linked = tmp_path / "linked.bin"
    linked.symlink_to(path.name)
    with pytest.raises(module.SourcePathListError, match="regular file"):
        module.read_source_path_list(linked)
    with pytest.raises(FileNotFoundError):
        module.read_source_path_list(tmp_path / "missing.bin")
    with pytest.raises(FileExistsError):
        module.write_source_path_list(path, ["a.txt"])
