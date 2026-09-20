"""Exact-request retry tests with disposable executables; no Cargo or live network."""

import contextlib
import copy
import hashlib
import io
import os
from pathlib import Path
import stat
import sys
import unittest
from unittest.mock import patch

import test_taira_release as existing
from taira_fake_libtest import executable

release = existing.release
gate = existing.development_gate


class IndependentCheckpointTests(unittest.TestCase):
    def setUp(self):
        self.fixture = existing.TairaPrepareTests()
        self.fixture.setUp()
        # This disposable census replaces the full stage groups; basic has a
        # separate admission/startup and network census that is not mocked here.
        self.fixture.args.native_check_scope = "full"
        self.addCleanup(self.fixture.tearDown)
        self.fixture.source.mkdir(mode=0o700)
        self.root = self.fixture.root
        self.executed = self.root / "executed"
        self.failures = self.root / "failed-tests"
        self.failures.write_text("")
        self.originals = self.fixture.target / "debug/deps"
        self.originals.parent.mkdir(mode=0o700)
        self.originals.mkdir(mode=0o700)
        self.artifacts = {}
        self.observations = []
        self.released = []
        for selection in ("core", "cli", "network"):
            path = self.originals / selection
            tests = [selection + "_first", selection + "_extra"]
            path.write_text(executable(tests, self.executed, failure_file=self.failures))
            path.chmod(0o700)
            self.artifacts[selection] = {
                "name": gate.HARNESS_TARGETS[selection][0], "executable": str(path),
                "profile": {"test": True},
                "manifest_path": str(self.fixture.source / "crates" / gate.HARNESS_TARGETS[selection][3][1] / "Cargo.toml"),
            }
        self.actual_checks = gate.run_checks
        shipping = patch.object(gate, "shipping_harnesses", return_value=())
        shipping.start()
        self.addCleanup(shipping.stop)
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        stack.enter_context(patch.dict(os.environ, {"PATH": "/usr/bin:/bin", "CARGO_HOME": str(self.root / "cargo-home")}, clear=True))
        self.source_lock = stack.enter_context((self.root / "source-lock").open("w"))
        self.events = []
        for group in tuple(name for name in vars(gate)
                           if name == "STAGES" or name.endswith("_STAGES")):
            stack.enter_context(patch.object(gate, group, ()))
        for group, selection in (("CORE_STAGES", "core"), ("STAGES", "cli"), ("NETWORK_STAGES", "network")):
            stack.enter_context(patch.object(gate, group, ((selection, (selection + "_first",)),)))
        for function in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "run_config_checks",
                         "require_network_fixture_capacity", "check_test_harnesses"):
            stack.enter_context(patch.object(gate, function))
        # Compilation alone is mocked; the exact-copy, custody/release, census,
        # subprocess result checks, preparation records and retry path are real.
        stack.enter_context(patch.object(gate, "NETWORK_FIXTURE_FREE_BYTES", 0))
        stack.enter_context(patch.object(gate, "native_artifact_guard", side_effect=lambda *_: contextlib.nullcontext()))
        self.compile = stack.enter_context(patch.object(gate, "compile_test_harnesses", side_effect=self.compile_fixtures))
        self.network = stack.enter_context(patch.object(gate, "run_network_checks", side_effect=self.run_network_fixture))
        stack.enter_context(contextlib.redirect_stderr(io.StringIO()))

    @property
    def checkpoint(self):
        return self.fixture.out / "independent-checks.json"

    def compile_fixtures(self, root, env, *, lock_fds, harnesses):
        self.events.append("combined compile")
        self.assertEqual(harnesses, ("core", "network", "cli"))
        request = release.read_record(self.fixture.out / "request.json")
        self.assertEqual(request["native_environment_sha256"],
                         hashlib.sha256(release.canonical_json_bytes(env)).hexdigest())
        copies = gate.isolate_native_artifacts(root, env, {name: self.artifacts[name] for name in harnesses})
        self.observations.append(copy.deepcopy(copies.observations))
        actual_release = copies.release

        def release_copy(selection):
            actual_release(selection)
            self.released.append(selection)

        copies.release = release_copy
        return copies

    def run_network_fixture(self, _root, fixture_root, env, lock_fds, *, harness, stages):
        self.events.append("network")
        self.assertTrue(self.checkpoint.exists(), "independent pass must precede real network execution")
        self.assertFalse((self.fixture.out / "checks.json").exists())
        self.assertEqual(stages, gate.NETWORK_STAGES)
        gate.run_stages(harness, fixture_root, env, stages, lock_fds)

    def prepare(self, **kwargs):
        return self.fixture.prepare(check=self.actual_checks, source_lane_fd=self.source_lock.fileno(), **kwargs)

    def ran(self):
        return self.executed.read_text().splitlines() if self.executed.exists() else []

    def fail_network_once(self):
        self.failures.write_text("network_first\n")
        with self.assertRaisesRegex(release.PrepareError, "network_first"):
            self.prepare()
        self.assertEqual(self.ran(), ["cli_first", "core_first", "network_first"])
        self.assertTrue(self.checkpoint.is_file())
        self.assertEqual(stat.S_IMODE(self.checkpoint.stat().st_mode), 0o400)
        self.assertFalse((self.fixture.out / "checks.json").exists())
        self.assertFalse((self.fixture.out / "result.json").exists())
        self.assertFalse(list(self.fixture.out.glob("attempts/*/cargo.log")))
        self.failures.write_text("")

    def test_network_failure_retries_real_network_without_repeating_independent_tests(self):
        self.fail_network_once()
        original = self.checkpoint.read_bytes()
        result, _, build = self.prepare()
        self.assertEqual(self.ran(), ["cli_first", "core_first", "network_first", "network_first"])
        self.assertEqual(self.events, ["combined compile", "network", "combined compile", "network"])
        self.assertEqual(build.call_count, 1)
        self.assertEqual(result["attempt"], "attempts/000002")
        self.assertFalse(result["deployed"])
        self.assertFalse(result["release_qualified"])
        self.assertEqual(self.checkpoint.read_bytes(), original)
        self.assertNotEqual(self.observations[0][0]["path"], self.observations[1][0]["path"])
        self.assertTrue(all(not Path(row["path"]).exists() for batch in self.observations for row in batch))

    def test_changed_source_request_cannot_reuse_checkpoint(self):
        self.fail_network_once()
        self.fixture.args.expected_commit = "c" * 40
        with self.assertRaisesRegex(release.PrepareError, "different inputs"):
            self.prepare()
        self.assertEqual(self.compile.call_count, 1)
        self.assertEqual(self.network.call_count, 1)

    def test_changed_toolchain_request_cannot_reuse_checkpoint(self):
        self.fail_network_once()
        self.fixture.zig.write_bytes(b"replacement reviewed fixture compiler")
        self.fixture.args.zig_sha256 = hashlib.sha256(self.fixture.zig.read_bytes()).hexdigest()
        with self.assertRaisesRegex(release.PrepareError, "different inputs"):
            self.prepare()
        self.assertEqual(self.compile.call_count, 1)
        self.assertEqual(self.network.call_count, 1)

    def test_changed_session_environment_restores_independent_checkpoint_inputs(self):
        self.fail_network_once()
        original = self.checkpoint.read_bytes()
        for key in ("HOME", "PATH", "TMPDIR", "TMP", "TEMP", "RUSTUP_HOME", "SCCACHE_DIR", "SCCACHE_CACHE_SIZE"):
            with self.subTest(key=key), patch.dict(os.environ, {key: "changed-fixture-environment-" + key}):
                self.prepare()
        self.assertEqual(self.compile.call_count, 2)
        self.assertEqual(self.network.call_count, 2)
        self.assertEqual(self.ran(), ["cli_first", "core_first", "network_first", "network_first"])
        self.assertEqual(self.checkpoint.read_bytes(), original)
        self.assertTrue((self.fixture.out / "checks.json").exists())

    def test_changed_session_environment_restores_full_checkpoint_after_linux_failure(self):
        def fail_linux(_root, _command, _env, log):
            log.write_text("fixture compiler failed")
            raise release.PrepareError("fixture Linux failure")

        with self.assertRaisesRegex(release.PrepareError, "fixture Linux failure"):
            self.prepare(build=fail_linux)
        self.assertTrue((self.fixture.out / "checks.json").exists())
        for key in ("HOME", "PATH", "TMPDIR", "TMP", "TEMP"):
            with self.subTest(key=key), patch.dict(os.environ, {key: "changed-fixture-environment-" + key}):
                self.prepare()
        self.assertEqual(len(list((self.fixture.out / "attempts").iterdir())), 2)
        self.assertEqual(self.compile.call_count, 1)

    def test_changed_session_environment_reuses_completed_capture_with_recorded_inputs(self):
        self.prepare()
        for key in ("HOME", "PATH", "TMPDIR", "TMP", "TEMP"):
            with self.subTest(key=key), patch.dict(os.environ, {key: "changed-fixture-environment-" + key}):
                self.prepare()
        self.assertEqual(len(list((self.fixture.out / "attempts").iterdir())), 1)
        self.assertEqual(self.compile.call_count, 1)

    def test_public_records_contain_only_digest_and_private_environment_excludes_runtime_secrets(self):
        values = {key: "private-runtime-value-fixture-" + key for key in ("HOME", "PATH", "TMPDIR", "TMP", "TEMP")}
        values["ONBOARDING_TOKEN"] = "runtime-secret-fixture-not-recorded"
        with patch.dict(os.environ, values):
            self.prepare()
        request = release.read_record(self.fixture.out / "request.json")
        environment_path = self.fixture.out / "environment.json"
        environment = release.read_record(environment_path)
        self.assertEqual(environment_path.stat().st_nlink, 1)
        self.assertEqual(stat.S_IMODE(environment_path.stat().st_mode), 0o400)
        self.assertEqual(hashlib.sha256(environment_path.read_bytes()).hexdigest(), request["environment_sha256"])
        self.assertNotIn("ONBOARDING_TOKEN", environment["child_environment"])
        self.assertNotIn(values["ONBOARDING_TOKEN"], environment_path.read_text())
        self.assertRegex(request["native_environment_sha256"], r"^[a-f0-9]{64}$")
        for path in (self.fixture.out / "request.json", self.checkpoint,
                     self.fixture.out / "checks.json", self.fixture.out / "result.json",
                     self.fixture.out / "attempts/000001/capture.json"):
            record = release.read_record(path)
            base = record["request"] if "request" in record else record
            self.assertEqual(base["native_environment_sha256"], request["native_environment_sha256"])
            self.assertEqual(base["environment_sha256"], request["environment_sha256"])
            for value in values.values():
                self.assertNotIn(value, path.read_text())

    def test_changed_selector_census_reruns_independent_tests(self):
        self.fail_network_once()
        with patch.object(gate, "CORE_STAGES", (("core", ("core_first", "core_extra")),)):
            self.prepare()
        self.assertEqual(self.ran()[3:], ["cli_first", "core_first", "core_extra", "network_first"])
        self.assertTrue((self.fixture.out / "attempts/000002/retired-independent-checks.json").is_file())

    def test_changed_binary_cannot_reuse_success_and_failed_rerun_cannot_revive_it(self):
        self.fail_network_once()
        path = Path(self.artifacts["core"]["executable"])
        original = path.read_bytes()
        path.write_bytes(original + b"# changed compiler output\n")
        self.failures.write_text("core_first\ncli_first\n")
        with self.assertRaisesRegex(release.PrepareError, "2 selected regressions failed"):
            self.prepare()
        self.assertFalse(self.checkpoint.exists())
        self.assertEqual(self.network.call_count, 1)
        self.assertEqual(self.ran()[3:], ["cli_first", "core_first"])
        self.assertTrue((self.fixture.out / "attempts/000002/retired-independent-checks.json").is_file())
        path.write_bytes(original)
        self.failures.write_text("")
        self.prepare()
        self.assertEqual(self.ran()[5:], ["cli_first", "core_first", "network_first"])

    def test_changed_actual_cargo_target_metadata_reruns_independent_tests(self):
        self.fail_network_once()
        self.artifacts["cli"]["profile"]["opt_level"] = "1"
        self.prepare()
        self.assertEqual(self.ran()[3:], ["cli_first", "core_first", "network_first"])

    def replace_record(self, record):
        self.checkpoint.rename(self.fixture.out / "retained-fixture-checkpoint.json")
        release.write_record(self.checkpoint, record)

    def test_incomplete_evidence_reruns_and_only_complete_evidence_is_republished(self):
        self.fail_network_once()
        record = release.read_record(self.checkpoint)
        record["evidence"].pop("artifacts")
        self.replace_record(record)
        self.prepare()
        self.assertEqual(self.ran()[3:], ["cli_first", "core_first", "network_first"])
        self.assertEqual(len(release.read_record(self.checkpoint)["evidence"]["artifacts"]), 2)

    def test_incomplete_record_envelope_stops_before_native_or_linux_work(self):
        self.fail_network_once()
        record = release.read_record(self.checkpoint)
        record.pop("request")
        self.replace_record(record)
        with self.assertRaisesRegex(release.PrepareError, "differs or is incomplete"):
            self.prepare()
        self.assertEqual(self.compile.call_count, 1)
        self.assertEqual(self.network.call_count, 1)
        self.assertFalse((self.fixture.out / "checks.json").exists())

    def test_source_drift_before_checkpoint_publication_prevents_network_and_linux(self):
        run_stages = gate.run_stages
        changed = False

        def change_source_after_tests(harness, *args, **kwargs):
            nonlocal changed
            run_stages(harness, *args, **kwargs)
            if Path(harness).name == "core":
                changed = True

        with patch.object(gate, "run_stages", side_effect=change_source_after_tests):
            with self.assertRaisesRegex(release.PrepareError, "captured source changed"):
                self.prepare(snapshot=lambda _: [{"path": "changed"}] if changed else [])
        self.assertEqual(self.ran(), ["cli_first", "core_first"])
        self.assertFalse(self.checkpoint.exists())
        self.network.assert_not_called()
        self.assertFalse((self.fixture.out / "checks.json").exists())

    def test_toolchain_drift_before_checkpoint_publication_prevents_network_and_linux(self):
        run_stages = gate.run_stages

        def change_tool_after_tests(harness, *args, **kwargs):
            run_stages(harness, *args, **kwargs)
            if Path(harness).name == "cli":
                self.fixture.zig.write_bytes(b"changed tool during checks")

        with patch.object(gate, "run_stages", side_effect=change_tool_after_tests):
            with self.assertRaisesRegex(release.PrepareError, "reviewed executable"):
                self.prepare()
        self.assertEqual(self.ran(), ["cli_first", "core_first"])
        self.assertFalse(self.checkpoint.exists())
        self.network.assert_not_called()
        self.assertFalse((self.fixture.out / "checks.json").exists())

    def test_fingerprint_retirement_removes_independent_success_before_failed_rerun(self):
        self.fail_network_once()
        old_record = self.checkpoint.read_bytes()

        def retire(*_args, before_retire):
            before_retire()
            self.assertFalse(self.checkpoint.exists())
            return ["ivm"]

        self.failures.write_text("core_first\n")
        with self.assertRaisesRegex(release.PrepareError, "core_first"):
            self.prepare(cache_admission=retire)
        self.assertFalse(self.checkpoint.exists())
        self.assertEqual((self.fixture.out / "attempts/000002/retired-independent-checks.json").read_bytes(), old_record)
        self.assertEqual(self.network.call_count, 1)

    def test_fingerprint_retirement_removes_both_checkpoints_after_linux_failure(self):
        def fail_linux(_root, _command, _env, log):
            log.write_text("fixture compiler failed")
            raise release.PrepareError("fixture Linux failure")

        with self.assertRaisesRegex(release.PrepareError, "fixture Linux failure"):
            self.prepare(build=fail_linux)
        checks = self.fixture.out / "checks.json"
        old_independent, old_checks = self.checkpoint.read_bytes(), checks.read_bytes()

        def retire(*_args, before_retire):
            before_retire()
            self.assertFalse(self.checkpoint.exists())
            self.assertFalse(checks.exists())
            return ["ivm"]

        self.failures.write_text("core_first\n")
        with self.assertRaisesRegex(release.PrepareError, "core_first"):
            self.prepare(cache_admission=retire)
        self.assertFalse(self.checkpoint.exists())
        self.assertFalse(checks.exists())
        attempt = self.fixture.out / "attempts/000002"
        self.assertEqual((attempt / "retired-independent-checks.json").read_bytes(), old_independent)
        self.assertEqual((attempt / "retired-checks.json").read_bytes(), old_checks)
        self.assertFalse((attempt / "cargo.log").exists())
        self.assertEqual(self.network.call_count, 1)


if __name__ == "__main__":
    unittest.main()
