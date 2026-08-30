"""Tests for the AtomicPrivateSettlementV1 source-evidence producer."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_source_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_source_evidence", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

VERIFIER_SCRIPT = ROOT / "scripts" / "private_settlement_release_evidence.py"
VERIFIER_SPEC = importlib.util.spec_from_file_location(
    "private_settlement_release_evidence_for_source_producer", VERIFIER_SCRIPT
)
assert VERIFIER_SPEC is not None and VERIFIER_SPEC.loader is not None
VERIFIER = importlib.util.module_from_spec(VERIFIER_SPEC)
sys.modules[VERIFIER_SPEC.name] = VERIFIER
VERIFIER_SPEC.loader.exec_module(VERIFIER)


def run_git(repository: Path, *arguments: str, text: bool = True):
    """Run one isolated Git fixture command."""

    environment = os.environ.copy()
    for name in tuple(environment):
        if name.startswith("GIT_"):
            environment.pop(name, None)
    environment["GIT_CONFIG_NOSYSTEM"] = "1"
    environment["GIT_CONFIG_GLOBAL"] = os.devnull
    return subprocess.run(
        ["git", *arguments],
        cwd=repository,
        check=True,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=text,
    ).stdout


def create_repository(
    parent: Path,
    name: str = "source",
    *,
    object_format: str | None = None,
) -> Path:
    """Create one small clean release checkout with varied Git modes."""

    repository = parent / name
    repository.mkdir()
    init_arguments = ["init", "--quiet"]
    if object_format is not None:
        init_arguments.append(f"--object-format={object_format}")
    run_git(repository, *init_arguments)
    run_git(repository, "config", "user.name", "APS Source Fixture")
    run_git(repository, "config", "user.email", "aps-source@example.invalid")
    run_git(repository, "config", "commit.gpgsign", "false")
    (repository / "Cargo.lock").write_text(
        "# exact APS fixture lockfile\nversion = 4\n", encoding="utf-8"
    )
    (repository / "README.md").write_text("APS source fixture\n", encoding="utf-8")
    for source_path in (
        *VERIFIER._FORMAL_SOURCE_PATHS,
        *VERIFIER._FORMAL_EVIDENCE_CODE_SOURCE_PATHS,
    ):
        path = repository / source_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"# exact APS fixture for {source_path}\n", encoding="utf-8")
        if path.suffix == ".sh":
            path.chmod(0o755)
    tools = repository / "tools"
    tools.mkdir()
    executable = tools / "qualify.sh"
    executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    executable.chmod(0o755)
    (repository / "lock-link").symlink_to("Cargo.lock")
    run_git(repository, "add", "--all")
    run_git(
        repository,
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "-m",
        "APS source fixture",
    )
    return repository


def artifact_bytes(bundle: Path, artifact: dict[str, object]) -> bytes:
    """Read and authenticate one producer-returned artifact."""

    path = bundle.joinpath(*str(artifact["path"]).split("/"))
    payload = path.read_bytes()
    if len(payload) != artifact["bytes"]:
        raise AssertionError(f"artifact byte count differs: {path}")
    if hashlib.sha256(payload).hexdigest() != artifact["sha256"]:
        raise AssertionError(f"artifact checksum differs: {path}")
    return payload


class PrivateSettlementSourceEvidenceTests(unittest.TestCase):
    """Exercise clean capture, exact inventory, and fail-closed behavior."""

    def test_produces_replayable_clean_tree_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = create_repository(root)
            bundle = root / "bundle"
            bundle.mkdir()

            summary = MODULE.produce_source_evidence(
                repository,
                bundle,
                "evidence/source",
                command="python3 scripts/private_settlement_source_evidence.py --fixture",
            )

            output = bundle / "evidence" / "source"
            self.assertTrue(output.is_dir())
            self.assertEqual(summary["protocol"], MODULE.PROTOCOL)
            artifacts = {
                item["kind"]: item
                for item in summary["artifacts"]
                if item["kind"] != "operator_log"
            }
            operator_logs = [
                item for item in summary["artifacts"] if item["kind"] == "operator_log"
            ]
            source_log = next(
                item for item in operator_logs if item["path"].endswith("source-capture.log")
            )
            inventory_log = next(
                item
                for item in operator_logs
                if item["path"].endswith("release-inventory.log")
            )
            self.assertEqual(len(summary["artifacts"]), 8)
            self.assertEqual(
                {item["kind"] for item in summary["artifacts"]},
                {
                    "source_path_list",
                    "source_archive",
                    "source_commit",
                    "source_lockfile",
                    "operator_log",
                    "source_manifest",
                    "release_inventory_report",
                },
            )
            for artifact in summary["artifacts"]:
                artifact_bytes(bundle, artifact)

            verifier_artifacts = {
                PurePosixPath(str(item["path"])): VERIFIER.Artifact(
                    kind=str(item["kind"]),
                    path=PurePosixPath(str(item["path"])),
                    sha256=str(item["sha256"]),
                    bytes=int(item["bytes"]),
                )
                for item in summary["artifacts"]
            }

            source_manifest = json.loads(
                artifact_bytes(bundle, artifacts["source_manifest"])
            )
            inventory_report = json.loads(
                artifact_bytes(bundle, artifacts["release_inventory_report"])
            )
            identity = MODULE._SOURCE_TOOLS.release_source_identity(repository)
            self.assertEqual(source_manifest["commit"], identity["head_commit"])
            self.assertEqual(source_manifest["tree"], identity["head_tree"])
            self.assertEqual(
                source_manifest["workspace_manifest_sha256"],
                identity["workspace_source_manifest_sha256"],
            )
            self.assertTrue(source_manifest["worktree_clean"])
            self.assertEqual(source_manifest["modified"], [])
            self.assertEqual(source_manifest["untracked"], [])
            self.assertEqual(inventory_report["gate"], "release_inventory")
            self.assertTrue(inventory_report["passed"])

            tree_raw = run_git(
                repository,
                "ls-tree",
                "-r",
                "-z",
                "--full-tree",
                str(identity["head_tree"]),
                text=False,
            )
            expected_entries = MODULE._parse_inventory(
                tree_raw, oid_hex_chars=len(str(identity["head_tree"]))
            )
            self.assertEqual(inventory_report["details"]["entries"], expected_entries)
            self.assertEqual(
                source_manifest["tracked_file_count"], len(expected_entries)
            )
            self.assertTrue(
                all(
                    set(entry) == {"path", "mode", "object_type", "object_id"}
                    for entry in expected_entries
                )
            )

            paths = MODULE._SOURCE_TOOLS.read_source_path_list(
                bundle.joinpath(*artifacts["source_path_list"]["path"].split("/"))
            )
            self.assertEqual(paths, [entry["path"] for entry in expected_entries])
            for reference_name, kind in (
                ("source_path_list", "source_path_list"),
                ("source_archive", "source_archive"),
                ("source_commit", "source_commit"),
                ("source_lockfile", "source_lockfile"),
            ):
                self.assertEqual(source_manifest[reference_name], {
                    key: artifacts[kind][key] for key in ("path", "sha256", "bytes")
                })
            self.assertEqual(
                source_manifest["transcript"],
                {key: source_log[key] for key in ("path", "sha256", "bytes")},
            )
            self.assertEqual(
                inventory_report["transcript"],
                {key: inventory_log[key] for key in ("path", "sha256", "bytes")},
            )

            raw_commit = artifact_bytes(bundle, artifacts["source_commit"])
            self.assertEqual(
                raw_commit,
                run_git(
                    repository,
                    "cat-file",
                    "commit",
                    str(identity["head_commit"]),
                    text=False,
                ),
            )
            self.assertEqual(
                artifact_bytes(bundle, artifacts["source_lockfile"]),
                (repository / "Cargo.lock").read_bytes(),
            )
            transcript = artifact_bytes(bundle, source_log).decode()
            self.assertIn("identity_command=compute_workspace_source_manifest.py", transcript)
            self.assertIn("post_capture_identity=unchanged", transcript)
            self.assertIn("passed=true", transcript)
            inventory_transcript = artifact_bytes(bundle, inventory_log).decode()
            self.assertIn(
                "inventory_command=git ls-tree -r -z --full-tree",
                inventory_transcript,
            )
            self.assertIn(
                "inventory_schema=path,mode,object_type,object_id",
                inventory_transcript,
            )

            (
                source_transcript,
                source_tree,
                source_count,
                workspace_manifest_sha256,
                source_archive_reference,
                source_commit_reference,
                source_lockfile_reference,
                source_path_list_reference,
            ) = VERIFIER._validate_source_manifest(
                bundle.joinpath(*PurePosixPath(artifacts["source_manifest"]["path"]).parts),
                commit=str(identity["head_commit"]),
                expected_sha256=str(artifacts["source_manifest"]["sha256"]),
                expected_bytes=int(artifacts["source_manifest"]["bytes"]),
                artifacts_by_path=verifier_artifacts,
            )
            self.assertEqual(source_transcript, PurePosixPath(source_log["path"]))
            self.assertEqual(source_tree, identity["head_tree"])
            self.assertEqual(source_count, len(expected_entries))
            source_commit_artifact = verifier_artifacts[source_commit_reference]
            self.assertEqual(
                VERIFIER._validate_source_commit(
                    bundle.joinpath(*source_commit_reference.parts),
                    str(identity["head_commit"]),
                    expected_sha256=source_commit_artifact.sha256,
                    expected_bytes=source_commit_artifact.bytes,
                ),
                identity["head_tree"],
            )
            source_archive_artifact = verifier_artifacts[source_archive_reference]
            source_lockfile_artifact = verifier_artifacts[source_lockfile_reference]
            source_path_list_artifact = verifier_artifacts[source_path_list_reference]
            inventory_transcript_path, formal_source_bindings = (
                VERIFIER._validate_pass_report(
                    bundle.joinpath(
                        *PurePosixPath(
                            artifacts["release_inventory_report"]["path"]
                        ).parts
                    ),
                    artifact_kind="release_inventory_report",
                    commit=str(identity["head_commit"]),
                    expected_sha256=str(
                        artifacts["release_inventory_report"]["sha256"]
                    ),
                    expected_bytes=int(
                        artifacts["release_inventory_report"]["bytes"]
                    ),
                    artifacts_by_path=verifier_artifacts,
                    source_tree=source_tree,
                    source_tracked_file_count=source_count,
                    source_archive_path=bundle.joinpath(
                        *source_archive_reference.parts
                    ),
                    source_archive_sha256=source_archive_artifact.sha256,
                    source_archive_bytes=source_archive_artifact.bytes,
                    workspace_manifest_sha256=workspace_manifest_sha256,
                    source_lockfile_sha256=source_lockfile_artifact.sha256,
                    source_lockfile_bytes=source_lockfile_artifact.bytes,
                    source_path_list_path=bundle.joinpath(
                        *source_path_list_reference.parts
                    ),
                    source_path_list_sha256=source_path_list_artifact.sha256,
                    source_path_list_bytes=source_path_list_artifact.bytes,
                )
            )
            self.assertEqual(
                inventory_transcript_path, PurePosixPath(inventory_log["path"])
            )
            self.assertIsNotNone(formal_source_bindings)
            assert formal_source_bindings is not None
            self.assertEqual(
                formal_source_bindings.model_sha256,
                VERIFIER._formal_package_sha256_from_source_payloads(
                    {
                        source_path: (repository / source_path).read_bytes()
                        for source_path in VERIFIER._FORMAL_SOURCE_PATHS
                    }
                ),
            )
            self.assertEqual(
                formal_source_bindings.evidence_code_sha256,
                VERIFIER._formal_evidence_code_sha256_from_source_payloads(
                    {
                        source_path: (repository / source_path).read_bytes()
                        for source_path in (
                            VERIFIER._FORMAL_EVIDENCE_CODE_SOURCE_PATHS
                        )
                    }
                ),
            )

            extracted = root / "extracted"
            extracted.mkdir()
            detached_manifest = MODULE._SOURCE_TOOLS.extract_source_seal(
                bundle.joinpath(*artifacts["source_archive"]["path"].split("/")),
                bundle.joinpath(*artifacts["source_path_list"]["path"].split("/")),
                extracted,
                str(identity["workspace_source_manifest_sha256"]),
                str(artifacts["source_archive"]["sha256"]),
                str(artifacts["source_path_list"]["sha256"]),
            )
            self.assertEqual(
                detached_manifest, identity["workspace_source_manifest_sha256"]
            )

    def test_sha256_repository_and_gitlink_replay_through_independent_verifier(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            try:
                repository = create_repository(
                    root,
                    "source-sha256",
                    object_format="sha256",
                )
            except subprocess.CalledProcessError as error:
                self.skipTest(f"installed Git does not support SHA-256 repositories: {error}")

            gitlink = repository / "vendor" / "dependency"
            gitlink.mkdir(parents=True)
            linked_commit = run_git(repository, "rev-parse", "HEAD").strip()
            run_git(
                repository,
                "update-index",
                "--add",
                "--cacheinfo",
                f"160000,{linked_commit},vendor/dependency",
            )
            run_git(
                repository,
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "add empty gitlink",
            )
            bundle = root / "bundle"
            bundle.mkdir()

            summary = MODULE.produce_source_evidence(repository, bundle)
            self.assertEqual(len(str(summary["commit"])), 64)
            self.assertEqual(len(str(summary["tree"])), 64)
            artifacts = {
                item["kind"]: item
                for item in summary["artifacts"]
                if item["kind"] != "operator_log"
            }
            source_manifest = json.loads(
                artifact_bytes(bundle, artifacts["source_manifest"])
            )
            inventory_report = json.loads(
                artifact_bytes(bundle, artifacts["release_inventory_report"])
            )
            gitlink_row = next(
                entry
                for entry in inventory_report["details"]["entries"]
                if entry["path"] == "vendor/dependency"
            )
            self.assertEqual(
                gitlink_row,
                {
                    "path": "vendor/dependency",
                    "mode": "160000",
                    "object_type": "commit",
                    "object_id": linked_commit,
                },
            )
            inventory = VERIFIER._validated_git_inventory(
                inventory_report["details"]["entries"],
                label="SHA-256 producer inventory",
                oid_hex_chars=64,
            )
            self.assertEqual(
                VERIFIER._git_inventory_tree_oid_v1(inventory, 64),
                summary["tree"],
            )
            archive = artifacts["source_archive"]
            lockfile = artifacts["source_lockfile"]
            VERIFIER._validate_source_seal_inventory(
                bundle.joinpath(*str(archive["path"]).split("/")),
                inventory,
                64,
                expected_sha256=str(archive["sha256"]),
                expected_bytes=int(archive["bytes"]),
                expected_workspace_manifest_sha256=str(
                    source_manifest["workspace_manifest_sha256"]
                ),
                expected_lockfile_sha256=str(lockfile["sha256"]),
                expected_lockfile_bytes=int(lockfile["bytes"]),
            )

    def test_rejects_dirty_index_worktree_and_untracked_paths(self) -> None:
        cases = ("staged", "tracked", "untracked")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for case in cases:
                with self.subTest(case=case):
                    repository = create_repository(root, f"source-{case}")
                    bundle = root / f"bundle-{case}"
                    bundle.mkdir()
                    if case == "staged":
                        (repository / "README.md").write_text(
                            "staged substitution\n", encoding="utf-8"
                        )
                        run_git(repository, "add", "README.md")
                    elif case == "tracked":
                        (repository / "README.md").write_text(
                            "unstaged substitution\n", encoding="utf-8"
                        )
                    else:
                        (repository / "untracked.txt").write_text(
                            "untracked source\n", encoding="utf-8"
                        )
                    with self.assertRaises(MODULE._SOURCE_TOOLS.DirtyReleaseSourceError):
                        MODULE.produce_source_evidence(repository, bundle)
                    self.assertFalse((bundle / "evidence" / "source").exists())
                    self.assertFalse((bundle / "evidence").exists())

    def test_rejects_committed_chained_symlink_escape_without_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = create_repository(root)
            (repository / "a").mkdir()
            (repository / "a" / "x").symlink_to("../b")
            (repository / "a" / "link").symlink_to("x/../../outside")
            run_git(repository, "add", "a/link", "a/x")
            run_git(
                repository,
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "add chained symlink escape",
            )
            bundle = root / "bundle"
            bundle.mkdir()

            with self.assertRaisesRegex(
                MODULE._SOURCE_TOOLS.SourceSealError,
                "chained out-of-root symlink",
            ):
                MODULE.produce_source_evidence(repository, bundle)

            self.assertFalse((bundle / "evidence" / "source").exists())
            evidence_parent = bundle / "evidence"
            self.assertEqual(list(evidence_parent.glob(".aps-source-evidence-*")), [])

    def test_lockfile_copy_uses_the_verifier_bound_and_cleans_failure(self) -> None:
        self.assertEqual(
            MODULE._MAX_LOCKFILE_BYTES,
            VERIFIER._MAX_SOURCE_LOCKFILE_BYTES,
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "Cargo.lock"
            with source.open("wb") as stream:
                stream.truncate(MODULE._MAX_LOCKFILE_BYTES + 1)
            destination = root / "copied.lock"

            with self.assertRaisesRegex(
                MODULE._SOURCE_TOOLS.SourceSealError,
                "bounded",
            ):
                MODULE._copy_lockfile(source, destination, "0" * 64)

            self.assertFalse(destination.exists())

    def test_rejects_malformed_or_noncanonical_tree_rows(self) -> None:
        object_id = b"0" * 40
        valid = b"100644 blob " + object_id + b"\tCargo.lock\0"
        malformed = (
            b"100644 tree " + object_id + b"\tCargo.lock\0",
            valid + valid,
            b"100644 blob " + object_id + b"\tbad-\xff\0",
            b"100644 blob " + object_id + b"\tz\0"
            + b"100644 blob "
            + object_id
            + b"\tCargo.lock\0",
        )
        for payload in malformed:
            with self.subTest(payload=payload):
                with self.assertRaises(MODULE.SourceEvidenceError):
                    MODULE._parse_inventory(payload, oid_hex_chars=40)

    def test_mid_capture_identity_change_is_not_published(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = create_repository(root)
            bundle = root / "bundle"
            bundle.mkdir()
            identity = MODULE._SOURCE_TOOLS.release_source_identity(repository)
            changed = dict(identity)
            changed["workspace_source_manifest_sha256"] = "0" * 64
            with mock.patch.object(
                MODULE._SOURCE_TOOLS,
                "release_source_identity",
                side_effect=(identity, changed),
            ):
                with self.assertRaisesRegex(
                    MODULE.SourceEvidenceError, "changed during evidence capture"
                ):
                    MODULE.produce_source_evidence(repository, bundle)
            self.assertFalse((bundle / "evidence" / "source").exists())
            evidence_parent = bundle / "evidence"
            self.assertEqual(list(evidence_parent.glob(".aps-source-evidence-*")), [])

    def test_refuses_output_inside_checkout_or_over_existing_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repository = create_repository(root)
            with self.assertRaisesRegex(
                MODULE.SourceEvidenceError, "outside the checkout"
            ):
                MODULE.produce_source_evidence(repository, repository)

            bundle = root / "bundle"
            existing = bundle / "evidence" / "source"
            existing.mkdir(parents=True)
            with self.assertRaisesRegex(
                MODULE.SourceEvidenceError, "already exists"
            ):
                MODULE.produce_source_evidence(repository, bundle)


if __name__ == "__main__":
    unittest.main()
