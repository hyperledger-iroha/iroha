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
ROOT = SCRIPT.parents[1]
HARNESS = (
    ROOT
    / "kotlin/kagemusha-candidate-evidence-lab/src/androidTest/java/"
    "org/hyperledger/iroha/sdk/kagemusha/candidate/lab/CandidateLabHarness.kt"
)
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
        self.qualification_receipt = b"canonical-recursive-step-two-qualification-v4\n"
        write_private(self.candidate / stage.CANDIDATE_RECORD_NAME, self.record)
        write_private(self.candidate / stage.CANDIDATE_JSON_NAME, b'{"candidate":"view"}\n')
        write_private(
            self.candidate / stage.CANDIDATE_SHA_NAME,
            f"{hashlib.sha256(self.record).hexdigest()}\n".encode("ascii"),
        )
        write_private(self.candidate / stage.ROSTER_NAME, self.roster)
        write_private(
            self.candidate / stage.QUALIFICATION_RECEIPT_NAME,
            self.qualification_receipt,
        )
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
        candidate_record_sha256 = hashlib.sha256(self.record).hexdigest()
        qualification_receipt_sha256 = hashlib.sha256(
            self.qualification_receipt
        ).hexdigest()
        return {
            "schema": stage.REPORT_SCHEMA,
            "candidate_record_sha256": candidate_record_sha256,
            "candidate_manifest_sha256": hashlib.sha256(self.manifest).hexdigest(),
            "qualification_receipt_file_name": stage.QUALIFICATION_RECEIPT_NAME,
            "qualification_receipt_sha256": qualification_receipt_sha256,
            "qualified_candidate_sha256": stage.qualified_candidate_sha256(
                candidate_record_sha256,
                qualification_receipt_sha256,
            ),
            "source_commit": SOURCE.commit,
            "source_tree_sha256": SOURCE.tree_sha256,
            "source_repo_dirty": False,
            "reviewed_source_closure_descriptor_sha256": "5" * 64,
            "authenticated_source_seal_projection_sha256": "6" * 64,
            "reviewed_cargo_binary_sha256": "7" * 64,
            "reviewed_rustc_binary_sha256": "8" * 64,
            "generation": "candidate.test.v4",
            "generation_memory_limit_bytes": 6 * 1024 * 1024 * 1024,
            "generation_memory_enforcement_profile": (
                stage.GENERATION_MEMORY_ENFORCEMENT_PROFILE
            ),
            "bridge_abi_version": 22,
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

    def test_descriptor_relative_publication_rejects_a_swapped_parent(self) -> None:
        parent = self.root / "publish-parent"
        parent.mkdir(mode=0o700)
        (parent / "staging").mkdir(mode=0o700)
        flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
        parent_fd = os.open(parent, flags)
        try:
            displaced = self.root / "displaced-parent"
            parent.rename(displaced)
            parent.mkdir(mode=0o700)
            (parent / "staging").mkdir(mode=0o700)
            sentinel = parent / "sentinel"
            sentinel.write_bytes(b"attacker sentinel")

            with self.assertRaisesRegex(stage.StageError, "parent changed"):
                stage._publish_noreplace(parent_fd, parent, "staging", "published")

            self.assertTrue((displaced / "staging").is_dir())
            self.assertTrue((parent / "staging").is_dir())
            self.assertEqual(sentinel.read_bytes(), b"attacker sentinel")
            self.assertFalse((parent / "published").exists())
        finally:
            os.close(parent_fd)

    def test_post_rename_fsync_failure_is_explicitly_uncertain(self) -> None:
        parent = self.root / "uncertain-parent"
        parent.mkdir(mode=0o700)
        (parent / "staging").mkdir(mode=0o700)
        flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
        parent_fd = os.open(parent, flags)
        try:
            with (
                mock.patch.object(stage.os, "fsync", side_effect=OSError("synthetic fsync fault")),
                self.assertRaisesRegex(stage.StagePublicationUncertain, "reached its final name"),
            ):
                stage._publish_noreplace(parent_fd, parent, "staging", "published")
            self.assertFalse((parent / "staging").exists())
            self.assertTrue((parent / "published").is_dir())
        finally:
            os.close(parent_fd)

    def test_authority_build_gates_both_targets_without_dropping_evidence_lab(self) -> None:
        command = stage._authority_build_command(Path("/toolchain/cargo"))
        packages = [
            command[index + 1]
            for index, argument in enumerate(command)
            if argument == "-p"
        ]
        binaries = [
            command[index + 1]
            for index, argument in enumerate(command)
            if argument == "--bin"
        ]
        features = set(command[command.index("--features") + 1].split(","))

        self.assertEqual(packages, ["iroha_core", "connect_norito_bridge"])
        self.assertEqual(
            binaries,
            [
                "kagemusha_recursive_spend_v4_bundle",
                "kagemusha_candidate_scenario_validator",
            ],
        )
        self.assertEqual(features, set(stage.AUTHORITY_BUILD_FEATURES))
        self.assertIn("iroha_core/dev-tools", features)
        self.assertIn("connect_norito_bridge/dev-tools", features)
        self.assertIn("iroha_core/kagemusha-candidate-evidence-lab", features)
        self.assertIn(
            "connect_norito_bridge/kagemusha-candidate-evidence-lab", features
        )

    def test_device_lifecycle_requires_exact_recursive_steps(self) -> None:
        source = HARNESS.read_text(encoding="utf-8")
        for exact_binding in (
            "init.branch.hopCount == 0 && init.branch.proofStepCount == 1",
            "hopOne.recipient.proofStepCount == 2",
            "hopOneChange.proofStepCount == 2",
            "hopTwo.recipient.proofStepCount == 3",
            "hopTwoChange.proofStepCount == 3",
            "requireVerified(verifyFirst, restoredHopOne.recipient.amount, 1, 2)",
            "requireVerified(verifyMultiHop, restoredHopTwo.recipient.amount, 2, 3)",
            "projection.proofStepCount == expectedProofSteps",
        ):
            self.assertIn(exact_binding, source)
        self.assertNotIn("minimumHops", source)

    def test_redeem_native_call_passes_taira_chain_discriminant(self) -> None:
        source = HARNESS.read_text(encoding="utf-8")
        call_start = source.index(
            "KagemushaCandidateLabNative.nativeBuildRedeemRequestV4("
        )
        call_prefix = source[call_start : call_start + 500]
        self.assertIn(
            "recipient,\n"
            "                TAIRA_I105_CHAIN_DISCRIMINANT,\n"
            "                atomicUnits,",
            call_prefix,
        )
        self.assertIn(
            "u32be(TAIRA_I105_CHAIN_DISCRIMINANT)",
            source,
        )
        self.assertIn("private const val TAIRA_I105_CHAIN_DISCRIMINANT = 369", source)

    def test_scenario_authority_pins_the_public_taira_discriminant(self) -> None:
        self.assertEqual(stage.PUBLIC_TAIRA_CHAIN_DISCRIMINANT, 369)
        source = SCRIPT.read_text(encoding="utf-8")
        invocation = source[source.index("scenario = subprocess.run(") :]
        self.assertIn('"--account-chain-discriminant"', invocation)
        self.assertIn("str(PUBLIC_TAIRA_CHAIN_DISCRIMINANT)", invocation)

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

    def test_report_parser_rejects_v1_and_unbound_qualification(self) -> None:
        self.fixture.report["schema"] = (
            "iroha.kagemusha.recursive_spend.candidate_validation." + "v1"
        )
        with self.assertRaisesRegex(stage.StageError, "schema is unsupported"):
            stage.parse_validation_report(self.report_bytes())
        self.fixture.report["schema"] = stage.REPORT_SCHEMA

        self.fixture.report["qualified_candidate_sha256"] = "f" * 64
        with self.assertRaisesRegex(stage.StageError, "qualified-candidate identity"):
            stage.parse_validation_report(self.report_bytes())

    def test_report_parser_rejects_noncanonical_reviewed_provenance_digests(self) -> None:
        fields = (
            "reviewed_source_closure_descriptor_sha256",
            "authenticated_source_seal_projection_sha256",
            "reviewed_cargo_binary_sha256",
            "reviewed_rustc_binary_sha256",
        )
        stage.parse_validation_report(self.report_bytes())
        for field in fields:
            with self.subTest(field=field, value="missing"):
                original = self.fixture.report.pop(field)
                with self.assertRaisesRegex(
                    stage.StageError,
                    "incomplete or excessive schema",
                ):
                    stage.parse_validation_report(self.report_bytes())
                self.fixture.report[field] = original
            with self.subTest(field=field, value="zero"):
                original = self.fixture.report[field]
                self.fixture.report[field] = "0" * 64
                with self.assertRaisesRegex(
                    stage.StageError,
                    "nonzero lowercase SHA-256",
                ):
                    stage.parse_validation_report(self.report_bytes())
                self.fixture.report[field] = original
        self.fixture.report[fields[0]] = "A" * 64
        with self.assertRaisesRegex(stage.StageError, "nonzero lowercase SHA-256"):
            stage.parse_validation_report(self.report_bytes())

    def test_report_parser_accepts_exact_artifact_limit_and_rejects_next_byte(self) -> None:
        maximum = 5 * 1024 * 1024 * 1024
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

    def test_source_identity_consumes_only_a_clean_signed_descriptor(self) -> None:
        descriptor = {
            "schema": "iroha.reviewed-source-closure.v1",
            "base_commit": SOURCE.commit,
            "source_commit": SOURCE.commit,
            "source_repo_dirty": False,
            "source_tree_sha256": SOURCE.tree_sha256,
            "tracked_binary_diff_sha256": stage.EMPTY_SHA256,
            "untracked_file_count": 0,
            "untracked_path_mode_blob_oid_manifest": [],
            "untracked_path_mode_blob_oid_manifest_sha256": stage.EMPTY_SHA256,
            "tracked_cargo_lock_size_bytes": 1,
            "tracked_cargo_lock_sha256": "3" * 64,
            "combined_source_fingerprint_sha256": "4" * 64,
        }
        clean = stage.subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout=stage._canonical_json(descriptor),
            stderr=b"",
        )
        dirty_descriptor = dict(descriptor)
        dirty_descriptor["source_repo_dirty"] = True
        dirty = stage.subprocess.CompletedProcess(
            args=[],
            returncode=0,
            stdout=stage._canonical_json(dirty_descriptor),
            stderr=b"",
        )
        with (
            mock.patch.object(stage, "_safe_system_environment", return_value={}),
            mock.patch.object(
                stage.subprocess,
                "run",
                side_effect=[clean, dirty],
            ) as runner,
        ):
            self.assertEqual(stage._source_identity(), SOURCE)
            self.assertIn("descriptor", runner.call_args_list[0].args[0])
            with self.assertRaisesRegex(stage.StageError, "not clean"):
                stage._source_identity()

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
            self.assertEqual(parsed["version"], 2)
            self.assertEqual(parsed["entry_count"], 45)
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
                (
                    output
                    / "evidence/candidate"
                    / stage.QUALIFICATION_RECEIPT_NAME
                ).read_bytes(),
                self.fixture.qualification_receipt,
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
