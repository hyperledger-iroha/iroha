"""Immutable ownership preflight ordering; no Cargo or live inputs."""

import contextlib
import io
from pathlib import Path
import unittest
from unittest.mock import patch

import test_taira_release_check as existing

gate = existing.gate


class Copies(dict):
    def __init__(self, events):
        super().__init__({name: "/copies/" + name for name in gate.MV_OWNERSHIP_HARNESSES})
        self.events = events

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.events.append("close")

    def release(self, name):
        self.events.append("release:" + name)


class ImmutableOwnershipPreflightTests(unittest.TestCase):
    def run_fixture(self, *, fail=False):
        events = []
        copies = Copies(events)
        scoped = {name: ((name, (name + "_test",)),)
                  for name in ("mv-map", "concread")}
        env = {"CARGO": "/unused/cargo", "CARGO_HOME": "/isolated",
               "CARGO_TARGET_DIR": "/warm"}

        def compile_batch(_root, _env, *, harnesses, lock_fds):
            events.append("compile:" + ",".join(harnesses))
            self.assertEqual(lock_fds, (77,))
            return copies

        def run(harness, fixture_root, runtime_env, stages, lock_fds):
            name = Path(harness).name
            events.append("run:" + name)
            self.assertEqual(fixture_root, Path("/warm"))
            self.assertEqual(runtime_env["IROHA_GIT_COMMIT_HASH"], "a" * 40)
            self.assertEqual(lock_fds, (77,))
            self.assertEqual(stages, scoped[name])
            if fail and name == "mv-map":
                raise gate.SelectedRegressionFailures(["map ownership failed"])

        with contextlib.ExitStack() as stack:
            existing.isolate_stage_fixture(stack)
            for name, stages in (("MV_MAP_STAGES", scoped["mv-map"]),
                                 ("CONCREAD_STAGES", scoped["concread"])):
                stack.enter_context(patch.object(gate, name, stages))
            for name in ("require_native_artifact_inspector",
                         "require_network_fixture_prerequisites",
                         "run_pure_fsm_checks", "run_lifecycle_source_checks",
                         "run_config_checks"):
                stack.enter_context(patch.object(gate, name))
            stack.enter_context(patch.object(gate, "shipping_harnesses", return_value=()))
            stack.enter_context(patch.object(gate, "check_test_harnesses",
                                      side_effect=lambda *_a, **_k: events.append("full-metadata")))
            stack.enter_context(patch.object(gate, "compile_test_harnesses", side_effect=compile_batch))
            stack.enter_context(patch.object(gate, "run_stages", side_effect=run))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            if fail:
                with self.assertRaisesRegex(gate.SelectedRegressionFailures, "map ownership failed"):
                    gate.run_checks(Path("/frozen"), environment=env,
                                    source_commit="a" * 40, lock_fds=(77,))
            else:
                gate.run_checks(Path("/frozen"), environment=env,
                                source_commit="a" * 40, lock_fds=(77,))
        return events

    def test_immutable_preflight_precedes_full_graph_and_full_graph_reruns_tests(self):
        self.assertEqual(self.run_fixture(), [
            "compile:mv-map,concread", "run:mv-map", "release:mv-map",
            "run:concread", "release:concread", "close", "full-metadata",
            "compile:mv-map,concread", "run:mv-map", "run:concread",
            "release:mv-map", "release:concread", "close",
        ])

    def test_failed_preflight_collects_failures_and_never_starts_full_graph(self):
        self.assertEqual(self.run_fixture(fail=True), [
            "compile:mv-map,concread", "run:mv-map", "release:mv-map",
            "run:concread", "release:concread", "close",
        ])

    def test_both_scopes_select_all_five_portable_ownership_harnesses(self):
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                stages = gate.qualification_stages(scope)
                self.assertEqual(tuple(name for name in gate.MV_OWNERSHIP_HARNESSES
                                       if stages[name]), gate.MV_OWNERSHIP_HARNESSES)
                names = [name for _, tests in stages["mv-map"] for name in tests]
                self.assertTrue({
                    "final_map_destruction_allocates_nothing_and_frees_every_original_layout",
                    "retained_reader_chain_and_final_tree_reclamation_do_not_allocate",
                    "final_detached_owner_reclaims_unpublished_nodes_and_retained_root_without_allocation",
                    "stale_detached_owner_reclaims_its_old_base_and_newer_committed_root_without_allocation",
                }.issubset(names))


if __name__ == "__main__":
    unittest.main()
