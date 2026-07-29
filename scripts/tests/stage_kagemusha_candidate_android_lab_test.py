from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "stage_kagemusha_candidate_android_lab.py"
SPEC = importlib.util.spec_from_file_location("stage_kagemusha_candidate_android_lab", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
stage = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = stage
SPEC.loader.exec_module(stage)


SOURCE = stage.SourceIdentity(commit="1" * 40, tree_sha256="2" * 64)


def write_private(path: Path, payload: bytes) -> None:
    path.write_bytes(payload)
    path.chmod(0o600)


class Fixture:
    def __init__(self, root: Path) -> None:
        self.candidate = root / "candidate"
        self.scenario = root / "scenario"
        self.output = root / "output"
        for directory in (self.candidate, self.scenario, self.output):
            directory.mkdir(mode=0o700)
            directory.chmod(0o700)
        self.record = b"canonical-candidate-v4\n"
        self.manifest = b"canonical-inner-manifest-v4\n"
        self.roster = b"canonical-real-roster-v2\n"
        write_private(self.candidate / stage.CANDIDATE_RECORD_NAME, self.record)
        write_private(self.candidate / stage.CANDIDATE_JSON_NAME, b'{"candidate":"view"}\n')
        write_private(
            self.candidate / stage.CANDIDATE_SHA_NAME,
            f"{hashlib.sha256(self.record).hexdigest()}\n".encode("ascii"),
        )
        write_private(self.candidate / stage.ROSTER_NAME, self.roster)
        self.artifact_payloads: dict[str, bytes] = {}
        for index, (_, name) in enumerate(stage.ARTIFACTS, start=1):
            payload = bytes([index]) * (64 + index)
            self.artifact_payloads[name] = payload
            write_private(self.candidate / name, payload)
        digest_index = 1
        for index, name in enumerate(stage.SCENARIO_FILES, start=1):
            if name == stage.SCENARIO_ROSTER_NAME:
                payload = self.roster
            elif name in stage.DIGEST_SEEDS:
                payload = digest_index.to_bytes(32, "big")
                digest_index += 1
            elif name in stage.POSITIVE_DECIMAL_SEEDS:
                payload = f"{1000 + index}\n".encode("ascii")
            elif name == "redeem-recipient-account-id.txt":
                payload = b"ed0120ABCDEF@wonderland\n"
            else:
                payload = f"real-seed-{index}-{name}\n".encode("ascii")
            write_private(self.scenario / name, payload)
        self.report = self.make_report()
        authority = root / "authority"
        authority.mkdir(mode=0o700)
        self.candidate_binary = authority / "kagemusha_recursive_spend_v4_bundle"
        self.scenario_binary = authority / "kagemusha_candidate_scenario_validator"
        write_private(self.candidate_binary, b"candidate-validator-binary\n")
        write_private(self.scenario_binary, b"scenario-validator-binary\n")
        self.candidate_binary.chmod(0o700)
        self.scenario_binary.chmod(0o700)
        candidate_binary_sha256 = hashlib.sha256(self.candidate_binary.read_bytes()).hexdigest()
        scenario_binary_sha256 = hashlib.sha256(self.scenario_binary.read_bytes()).hexdigest()
        self.validator_identity = {
            "schema": stage.VALIDATOR_SCHEMA,
            "candidate_binary_name": self.candidate_binary.name,
            "candidate_binary_sha256": candidate_binary_sha256,
            "scenario_binary_name": self.scenario_binary.name,
            "scenario_binary_sha256": scenario_binary_sha256,
            "cargo_binary_sha256": "3" * 64,
            "cargo_version_verbose": "cargo 1.93.1\ncommit-hash: test\n",
            "rustc_binary_sha256": "4" * 64,
            "rustc_version_verbose": "rustc 1.93.1\ncommit-hash: test\n",
            "locked": True,
            "offline": True,
            "isolated_target": True,
            "build_jobs": 2,
            "candidate_package": "iroha_core",
            "scenario_package": "connect_norito_bridge",
            "features": ["kagemusha-candidate-evidence-lab"],
            "profile": "debug",
        }
        self.authorities = stage.AuthorityBinaries(
            candidate=self.candidate_binary,
            scenario=self.scenario_binary,
            candidate_sha256=candidate_binary_sha256,
            scenario_sha256=scenario_binary_sha256,
            identity=self.validator_identity,
        )

    def make_report(self) -> dict[str, object]:
        artifacts = []
        for index, (role, name) in enumerate(stage.ARTIFACTS, start=1):
            payload = self.artifact_payloads[name]
            artifacts.append(
                {
                    "role": role,
                    "file_name": name,
                    "framed_size_bytes": len(payload),
                    "framed_sha256": hashlib.sha256(payload).hexdigest(),
                    "payload_size_bytes": len(payload) - 1,
                    "payload_sha256": hashlib.sha256(f"payload-{index}".encode()).hexdigest(),
                }
            )
        return {
            "schema": stage.REPORT_SCHEMA,
            "candidate_record_sha256": hashlib.sha256(self.record).hexdigest(),
            "candidate_manifest_sha256": hashlib.sha256(self.manifest).hexdigest(),
            "source_commit": SOURCE.commit,
            "source_tree_sha256": SOURCE.tree_sha256,
            "source_repo_dirty": True,
            "generation": "candidate.test.v4",
            "bridge_abi_version": 21,
            "artifact_count": 8,
            "artifacts": artifacts,
            "topup_finality_roster_file_name": stage.ROSTER_NAME,
            "topup_finality_roster_size_bytes": len(self.roster),
            "topup_finality_roster_sha256": hashlib.sha256(self.roster).hexdigest(),
        }

    def scenario_inventory(self) -> str:
        digest = hashlib.sha256(stage.SCENARIO_INVENTORY_DOMAIN)
        digest.update(len(stage.SCENARIO_FILES).to_bytes(4, "big"))
        for name in sorted(stage.SCENARIO_FILES):
            payload = (self.scenario / name).read_bytes()
            path = f"scenario/{name}".encode()
            digest.update(len(path).to_bytes(4, "big"))
            digest.update(path)
            digest.update(len(payload).to_bytes(8, "big"))
            digest.update(hashlib.sha256(payload).digest())
        return digest.hexdigest()

    def validator(
        self,
        _authorities: stage.AuthorityBinaries,
        _candidate: Path,
        _scenario: Path,
        output: Path,
    ) -> dict[str, object]:
        output.mkdir(mode=0o700)
        output.chmod(0o700)
        write_private(
            output / stage.REPORT_NAME,
            (json.dumps(self.report, sort_keys=True, separators=(",", ":")) + "\n").encode(),
        )
        write_private(output / stage.VALIDATED_MANIFEST_NAME, self.manifest)
        return {
            "schema": stage.SCENARIO_VALIDATION_SCHEMA,
            "candidate_record_sha256": hashlib.sha256(self.record).hexdigest(),
            "candidate_manifest_sha256": hashlib.sha256(self.manifest).hexdigest(),
            "scenario_inventory_sha256": self.scenario_inventory(),
            "scenario_file_count": len(stage.SCENARIO_FILES),
            "finalized_height": 100,
        }


class CandidateStagerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.fixture = Fixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def report_bytes(self) -> bytes:
        return (json.dumps(self.fixture.report, sort_keys=True) + "\n").encode()

    def test_report_parser_rejects_extra_fields_and_wrong_artifact_order(self) -> None:
        stage.parse_validation_report(self.report_bytes())
        duplicate = self.report_bytes().replace(
            b'"schema":', b'"schema":"duplicate","schema":', 1
        )
        with self.assertRaisesRegex(stage.StageError, "repeats JSON key"):
            stage.parse_validation_report(duplicate)
        self.fixture.report["extra"] = True
        with self.assertRaisesRegex(stage.StageError, "excessive schema"):
            stage.parse_validation_report(self.report_bytes())
        del self.fixture.report["extra"]
        artifacts = self.fixture.report["artifacts"]
        assert isinstance(artifacts, list)
        artifacts[0], artifacts[1] = artifacts[1], artifacts[0]
        with self.assertRaisesRegex(stage.StageError, "order or role"):
            stage.parse_validation_report(self.report_bytes())

    def test_report_parser_accepts_exact_artifact_limit_and_rejects_next_byte(self) -> None:
        maximum = 4 * 1024 * 1024 * 1024
        self.assertEqual(stage.MAX_ARTIFACT_BYTES, maximum)
        artifacts = self.fixture.report["artifacts"]
        assert isinstance(artifacts, list)
        artifact = artifacts[0]
        assert isinstance(artifact, dict)
        artifact["framed_size_bytes"] = maximum
        artifact["payload_size_bytes"] = maximum - 1
        stage.parse_validation_report(self.report_bytes())
        artifact["framed_size_bytes"] = maximum + 1
        with self.assertRaisesRegex(stage.StageError, "exceed the V4 corridor"):
            stage.parse_validation_report(self.report_bytes())

    def test_source_identity_race_is_rejected(self) -> None:
        changed = stage.SourceIdentity(commit="5" * 40, tree_sha256="6" * 64)
        completed = subprocess_completed()
        with (
            mock.patch.object(
                stage,
                "_run_git",
                return_value=(str(stage.REPOSITORY_ROOT) + "\n").encode(),
            ),
            mock.patch.object(stage, "_source_identity", side_effect=[SOURCE, changed]),
            mock.patch.object(stage.subprocess, "run", return_value=completed),
        ):
            with self.assertRaisesRegex(stage.StageError, "changed during"):
                stage.verify_current_source()

    def test_input_inventory_rejects_missing_extra_symlink_and_hardlink(self) -> None:
        missing = self.fixture.scenario / stage.SCENARIO_FILES[0]
        saved = missing.read_bytes()
        missing.unlink()
        with self.assertRaisesRegex(stage.StageError, "missing="):
            stage.OpenedDirectory(self.fixture.scenario, stage.SCENARIO_INVENTORY)
        write_private(missing, saved)

        write_private(self.fixture.scenario / "prebuilt-redeem-result.norito", b"forbidden\n")
        with self.assertRaisesRegex(stage.StageError, "extra="):
            stage.OpenedDirectory(self.fixture.scenario, stage.SCENARIO_INVENTORY)
        (self.fixture.scenario / "prebuilt-redeem-result.norito").unlink()

        target = self.fixture.scenario / stage.SCENARIO_FILES[0]
        target.unlink()
        os.symlink(stage.SCENARIO_FILES[1], target)
        with stage.OpenedDirectory(self.fixture.scenario, stage.SCENARIO_INVENTORY) as directory:
            with self.assertRaisesRegex(stage.StageError, "regular input"):
                directory.open(stage.SCENARIO_FILES[0], stage.MAX_SCENARIO_BYTES)
        target.unlink()
        write_private(target, saved)

        alias = self.fixture.scenario / stage.SCENARIO_FILES[2]
        alias.unlink()
        os.link(target, alias)
        with stage.OpenedDirectory(self.fixture.scenario, stage.SCENARIO_INVENTORY) as directory:
            with self.assertRaisesRegex(stage.StageError, "singly linked"):
                directory.open(stage.SCENARIO_FILES[0], stage.MAX_SCENARIO_BYTES)

    def test_scenario_inventory_requires_exact_files_and_binds_content(self) -> None:
        with stage.OpenedDirectory(self.fixture.scenario, stage.SCENARIO_INVENTORY) as directory:
            with contextlib_exit_stack() as stack:
                files = {
                    name: stack.enter_context(directory.open(name, stage.MAX_SCENARIO_BYTES))
                    for name in stage.SCENARIO_FILES
                }
                original = stage.scenario_inventory_sha256(files)
                incomplete = dict(files)
                incomplete.pop(stage.SCENARIO_FILES[0])
                with self.assertRaisesRegex(stage.StageError, "exact 33 files"):
                    stage.scenario_inventory_sha256(incomplete)

        changed = self.fixture.scenario / stage.SCENARIO_FILES[0]
        write_private(changed, changed.read_bytes() + b"changed\n")
        with stage.OpenedDirectory(self.fixture.scenario, stage.SCENARIO_INVENTORY) as directory:
            with contextlib_exit_stack() as stack:
                files = {
                    name: stack.enter_context(directory.open(name, stage.MAX_SCENARIO_BYTES))
                    for name in stage.SCENARIO_FILES
                }
                self.assertNotEqual(stage.scenario_inventory_sha256(files), original)

    def test_stages_exact_sha_rooted_layout_and_refuses_overwrite(self) -> None:
        with (
            mock.patch.object(stage, "OUTPUT_PARENT", self.fixture.output),
            mock.patch.object(stage, "verify_current_source", return_value=SOURCE),
            mock.patch.object(
                stage,
                "build_authoritative_validators",
                return_value=self.fixture.authorities,
            ),
            mock.patch.object(
                stage,
                "run_authoritative_validators",
                side_effect=self.fixture.validator,
            ),
        ):
            output = stage.stage_candidate(self.fixture.candidate, self.fixture.scenario)
            candidate_sha = hashlib.sha256(self.fixture.record).hexdigest()
            self.assertEqual(output.parent, self.fixture.output / candidate_sha)
            manifest_payload = (output / stage.STAGE_MANIFEST_NAME).read_bytes()
            self.assertEqual(output.name, hashlib.sha256(manifest_payload).hexdigest())
            parsed = stage.parse_stage_manifest(
                manifest_payload,
                root=output,
                expected_sha256=output.name,
            )
            self.assertEqual(parsed["entry_count"], 44)
            self.assertEqual(parsed["scenario_entry_count"], 33)
            self.assertEqual(
                (output / "evidence/candidate/candidate-v4.norito").read_bytes(),
                self.fixture.record,
            )
            self.assertEqual(
                (output / "evidence/candidate/manifest-v4.norito").read_bytes(),
                self.fixture.manifest,
            )
            self.assertEqual(
                {path.name for path in (output / "evidence/candidate/artifacts").iterdir()},
                set(stage.ARTIFACT_NAMES),
            )
            self.assertEqual(
                {path.name for path in (output / "scenario").iterdir()},
                set(stage.SCENARIO_FILES),
            )
            self.assertFalse((output / "scenario/append-request-v4.norito").exists())
            with self.assertRaisesRegex(stage.StageError, "already exists"):
                stage.stage_candidate(self.fixture.candidate, self.fixture.scenario)

    def test_stage_manifest_rejects_self_inclusion_and_extra_path(self) -> None:
        with (
            mock.patch.object(stage, "OUTPUT_PARENT", self.fixture.output),
            mock.patch.object(stage, "verify_current_source", return_value=SOURCE),
            mock.patch.object(
                stage,
                "build_authoritative_validators",
                return_value=self.fixture.authorities,
            ),
            mock.patch.object(
                stage,
                "run_authoritative_validators",
                side_effect=self.fixture.validator,
            ),
        ):
            output = stage.stage_candidate(self.fixture.candidate, self.fixture.scenario)
        manifest = json.loads((output / stage.STAGE_MANIFEST_NAME).read_text())

        def encoded(value: dict[str, object]) -> bytes:
            value["stage_manifest_size_bytes"] = 0
            for _ in range(16):
                payload = stage._canonical_json(value)
                if value["stage_manifest_size_bytes"] == len(payload):
                    return payload
                value["stage_manifest_size_bytes"] = len(payload)
            self.fail("manifest size did not converge")

        self_including = json.loads(json.dumps(manifest))
        self_including["entries"][0]["path"] = stage.STAGE_MANIFEST_NAME
        with self.assertRaisesRegex(stage.StageError, "unsafe/self"):
            stage.parse_stage_manifest(encoded(self_including))

        extra = json.loads(json.dumps(manifest))
        extra["entries"][0]["path"] = "evidence/candidate/extra.bin"
        with self.assertRaisesRegex(stage.StageError, "missing, extra"):
            stage.parse_stage_manifest(encoded(extra))

    def test_swapped_validator_binaries_and_ascii_authority_failure_never_publish(self) -> None:
        swapped = stage.AuthorityBinaries(
            candidate=self.fixture.scenario_binary,
            scenario=self.fixture.candidate_binary,
            candidate_sha256=self.fixture.authorities.scenario_sha256,
            scenario_sha256=self.fixture.authorities.candidate_sha256,
            identity=self.fixture.validator_identity,
        )
        with self.assertRaisesRegex(stage.StageError, "assignment"):
            stage.run_authoritative_validators(
                swapped,
                self.fixture.candidate,
                self.fixture.scenario,
                self.root / "unused-validation",
            )

        write_private(
            self.fixture.scenario / "init-opening-v2.norito",
            b"arbitrary ASCII is not Norito\n",
        )
        with (
            mock.patch.object(stage, "OUTPUT_PARENT", self.fixture.output),
            mock.patch.object(stage, "verify_current_source", return_value=SOURCE),
            mock.patch.object(
                stage,
                "build_authoritative_validators",
                return_value=self.fixture.authorities,
            ),
            mock.patch.object(
                stage,
                "run_authoritative_validators",
                side_effect=stage.StageError("not one canonical typed Norito archive"),
            ),
        ):
            with self.assertRaisesRegex(stage.StageError, "typed Norito"):
                stage.stage_candidate(self.fixture.candidate, self.fixture.scenario)
        self.assertEqual(list(self.fixture.output.iterdir()), [])


def contextlib_exit_stack():
    import contextlib

    return contextlib.ExitStack()


def subprocess_completed():
    import subprocess

    return subprocess.CompletedProcess(args=["git"], returncode=0)


if __name__ == "__main__":
    unittest.main()
