"""Guard the fixed native shards and exact aggregate without executing native code."""
from collections import Counter
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
INPUTS = "linux_resource_harness_inputs"
SHARDS = "linux_resource_harness_shards"
AGGREGATE = "linux_resource_harness"
SHARD_IDS = ("native-1", "native-2", "native-3", "native-4")
CHECKOUT = "actions/checkout@11d5960a326750d5838078e36cf38b85af677262"
DOWNLOAD = "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333"
UPLOAD = "actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9"
PYTHON = "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1"
SCRIPT = "harness/scripts/run_required_linux_resource_harness.py"
CONFIG = "observer-approval/ci/linux_observer_approval.json"


def job(text, name):
    matches = list(re.finditer(r"(?ms)^  " + re.escape(name) + r":\n(.*?)(?=^  [a-z_]+:|\Z)", text))
    if len(matches) != 1:
        raise AssertionError("require one exact job: " + name)
    return matches[0].group(1)


def step(body, name):
    matches = list(re.finditer(r"(?ms)^      - name: " + re.escape(name) + r"\n(.*?)(?=^      - |\Z)", body))
    if len(matches) != 1:
        raise AssertionError("require one exact step: " + name)
    return matches[0].group(1)


def command(body, name):
    selected = step(body, name)
    run = re.search(r"(?m)^        run: (.+)$", selected)
    if run is None:
        raise AssertionError("missing run command: " + name)
    return textwrap.dedent(selected[run.end() + 1:]) if run.group(1) == "|" else run.group(1)


class RequiredLinuxResourceHarnessWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/pr.yml").read_text()

    def assert_contract(self, workflow):
        inputs, shards, aggregate = (job(workflow, name) for name in (INPUTS, SHARDS, AGGREGATE))
        for body in (inputs, shards, aggregate):
            self.assertIn("    runs-on: ubuntu-24.04\n", body)
            self.assertIn("    timeout-minutes: 90\n", body)
            self.assertIn("      contents: read\n      actions: read\n", body)
            self.assertIn("        shell: bash\n", body)
            self.assertNotIn("continue-on-error:", body)
            self.assertIn('          python-version: "3.12"\n', body)
            self.assertIn("python -m pip install --disable-pip-version-check -r harness/scripts/requirements.txt", body)
            self.assertEqual(set(re.findall(r"(?m)^\s+(?:- )?uses: ([^\n]+)$", body)) - {CHECKOUT, DOWNLOAD, UPLOAD, PYTHON}, set())
            self.assertNotIn("persist-credentials: true", body)
            self.assertNotIn("eval ", body)
        for body, expected in ((inputs, Counter({CHECKOUT: 2, PYTHON: 1})),
                               (shards, Counter({CHECKOUT: 3, PYTHON: 1, DOWNLOAD: 1, UPLOAD: 1})),
                               (aggregate, Counter({CHECKOUT: 2, PYTHON: 1, DOWNLOAD: 1, UPLOAD: 1}))):
            self.assertEqual(Counter(re.findall(r"(?m)^\s+(?:- )?uses: ([^\n]+)$", body)), expected)
            self.assertEqual(re.findall(r"(?m)^        if: (.+)$", body), [] if body == inputs else ["always()"])
        self.assertEqual(workflow.count("${{ vars.LINUX_OBSERVER_APPROVAL_COMMIT }}"), 1)
        capture = step(inputs, "Capture independently selected immutable approval once")
        self.assertIn('[[ "$APPROVAL_COMMIT" =~ ^[0-9a-f]{40}$ ]]', capture)
        self.assertIn('[[ "$APPROVAL_COMMIT" != 0000000000000000000000000000000000000000 ]]', capture)
        self.assertIn("ref: ${{ steps.approval_ref.outputs.approval_commit }}", inputs)
        self.assertIn("HARNESS_COMMIT: ${{ github.sha }}", inputs)
        self.assertIn("HEAD_COMMIT: ${{ github.event.pull_request.head.sha || github.sha }}", inputs)
        for output in ("inputs_json", "approval_commit", "harness_commit", "head_commit", "repository", "producer_run_id", "producer_run_attempt", "artifact_id", "source_commit"):
            self.assertIn("      " + output + ": ${{ steps.inputs.outputs." + output + " }}\n", inputs)
        controls = "python -B -m unittest discover -s harness/scripts/tests -p 'required_linux_resource_harness*_test.py' -v"
        self.assertEqual(workflow.count(controls), 1)
        self.assertIn(controls, inputs)
        self.assertNotRegex(inputs, r"(?m)^    if:")
        self.assertNotRegex(shards, r"(?m)^    if:")
        self.assertIn("    needs: linux_resource_harness_inputs\n", shards)
        self.assertIn("    name: Linux native resource harness shard (${{ matrix.shard }})\n", shards)
        self.assertIn("      fail-fast: false\n      max-parallel: 4\n      matrix:\n        shard: [native-1, native-2, native-3, native-4]\n", shards)
        self.assertEqual(re.findall(r"(?m)^        shard: (.+)$", shards), ["[native-1, native-2, native-3, native-4]"])
        self.assertNotIn("include:", shards)
        self.assertNotIn("exclude:", shards)
        for body in (shards, aggregate):
            for field in ("harness_commit", "approval_commit"):
                self.assertIn("ref: ${{ needs.linux_resource_harness_inputs.outputs." + field + " }}", body)
            self.assertIn("CONTEXT_JSON: ${{ needs.linux_resource_harness_inputs.outputs.inputs_json }}", body)
            self.assertIn('json.loads(os.environ["CONTEXT_JSON"])', body)
            self.assertIn("skip-decompress: true", body)
            self.assertIn("digest-mismatch: error", body)
        self.assertIn("ref: ${{ steps.approval.outputs.source_commit }}", shards)
        self.assertIn('test "$RUNNER_ENVIRONMENT" = github-hosted', shards)
        self.assertIn('test "$(id -u)" -ne 0', shards)
        self.assertIn("sudo -n sysctl -w kernel.perf_event_paranoid=2", shards)
        self.assertIn('test "$(cat "$evidence_dir/perf-policy-after.txt")" = 2', shards)
        run = step(shards, "Run assigned actual native methods with zero skips")
        self.assertIn("SHARD: ${{ matrix.shard }}", run)
        self.assertIn('--shard "$SHARD"', run)
        self.assertNotIn("--test ", run)
        self.assertNotRegex(run, r"(?m)^        if:")
        upload = step(shards, "Preserve native results and original failure evidence")
        self.assertIn("        if: always()\n", upload)
        self.assertIn("name: linux-resource-harness-${{ github.run_id }}-${{ github.run_attempt }}-${{ matrix.shard }}\n", upload)
        self.assertIn("${{ runner.temp }}/native-observer-required/\n            ${{ runner.temp }}/native-observer-metadata/", upload)
        self.assertIn("    name: Required Linux native resource harness (20 tests)\n", aggregate)
        self.assertIn("    if: always()\n    needs: [linux_resource_harness_inputs, linux_resource_harness_shards]\n", aggregate)
        prerequisites = command(aggregate, "Require complete immutable inputs for aggregation")
        self.assertIn('test "$INPUTS_RESULT" = success', prerequisites)
        self.assertIn('test "$SHARDS_RESULT" = success', prerequisites)
        metadata = command(aggregate, "Retain original attempt-specific consumer jobs and artifacts")
        self.assertIn('gh api --paginate --slurp "repos/$CONSUMER_REPOSITORY/actions/runs/$CONSUMER_RUN_ID/attempts/$CONSUMER_RUN_ATTEMPT/jobs?per_page=100"', metadata)
        self.assertIn('gh api "repos/$CONSUMER_REPOSITORY/actions/runs/$CONSUMER_RUN_ID/attempts/$CONSUMER_RUN_ATTEMPT"', metadata)
        self.assertIn('gh api --paginate --slurp "repos/$CONSUMER_REPOSITORY/actions/runs/$CONSUMER_RUN_ID/artifacts?per_page=100"', metadata)
        download = step(aggregate, "Download exact shard artifacts without extracting them")
        self.assertIn("artifact-ids: ${{ steps.select.outputs.artifact_ids }}", download)
        self.assertNotIn("pattern:", download)
        self.assertNotIn("merge-multiple:", download)
        self.assertIn("run-id: ${{ github.run_id }}", download)
        final = step(aggregate, "Require all four same-attempt shards and exact 20 passing native methods")
        self.assertNotRegex(final, r"(?m)^        if:")
        self.assertIn(SCRIPT + " aggregate", final)
        self.assertIn('--jobs-metadata "$RUNNER_TEMP/native-observer-aggregate-metadata/jobs.json"', final)
        required = job(workflow, "rust_required")
        self.assertIn("sora_parliament_lifecycle, linux_resource_harness]", required)
        self.assertIn("NATIVE_HARNESS_RESULT: ${{ needs.linux_resource_harness.result }}", required)

    def test_fixed_topology_preserves_admission_and_required_aggregate(self):
        self.assert_contract(self.workflow)

    def test_closed_workflow_rejects_missing_shuffled_skipped_or_weakened_shards(self):
        mutations = {
            "missing_shard": ("[native-1, native-2, native-3, native-4]", "[native-1, native-2, native-3]"),
            "shuffled_shards": ("[native-1, native-2, native-3, native-4]", "[native-2, native-1, native-3, native-4]"),
            "skipped_shards": ("  linux_resource_harness_shards:\n", "  linux_resource_harness_shards:\n    if: false\n"),
            "fail_fast": ("      fail-fast: false\n      max-parallel: 4", "      fail-fast: true\n      max-parallel: 4"),
            "deadline": ("    timeout-minutes: 90\n", "    timeout-minutes: 91\n"),
            "selector": ("          SHARD: ${{ matrix.shard }}", "          SHARD: native-1"),
            "aggregate_skip": ("    if: always()\n    needs: [linux_resource_harness_inputs, linux_resource_harness_shards]", "    if: success()\n    needs: [linux_resource_harness_inputs, linux_resource_harness_shards]"),
            "aggregate_missing_dependency": ("needs: [linux_resource_harness_inputs, linux_resource_harness_shards]", "needs: [linux_resource_harness_inputs]"),
            "reloaded_approval": ("ref: ${{ needs.linux_resource_harness_inputs.outputs.approval_commit }}", "ref: ${{ vars.LINUX_OBSERVER_APPROVAL_COMMIT }}"),
            "decompressed_archive": ("skip-decompress: true", "skip-decompress: false"),
            "ignored_digest": ("digest-mismatch: error", "digest-mismatch: warn"),
            "missing_attempt": ("/attempts/$CONSUMER_RUN_ATTEMPT/jobs?per_page=100", "/jobs?per_page=100"),
        }
        for name, (old, new) in mutations.items():
            with self.subTest(mutation=name):
                self.assertIn(old, self.workflow)
                changed = self.workflow.replace(old, new)
                with self.assertRaises(AssertionError):
                    self.assert_contract(changed)

    def test_actual_consumer_shell_uses_frozen_arguments_and_exact_shard(self):
        env = dict(os.environ, GITHUB_WORKSPACE="/fixture/workspace", RUNNER_TEMP="/fixture/temp",
                   GITHUB_OUTPUT="/fixture/github-output", APPROVAL_COMMIT="a" * 40,
                   HARNESS_COMMIT="b" * 40, HEAD_COMMIT="c" * 40, CONSUMER_RUN_ID="123", CONSUMER_RUN_ATTEMPT="2")
        common = ["-B", SCRIPT]
        config = ["--config", CONFIG, "--harness-root", "/fixture/workspace/harness"]
        cases = [
            (INPUTS, "Freeze identical consumer and publication inputs for all shards", "native-1",
             [*common, "inputs", *config, "--approval-root", "/fixture/workspace/observer-approval",
              "--approval-commit", "a" * 40, "--harness-commit", "b" * 40, "--head-commit", "c" * 40,
              "--run-id", "123", "--run-attempt", "2", "--github-output", "/fixture/github-output"]),
            (AGGREGATE, "Select the exact four original shard artifacts", "native-1",
             [*common, "select", *config, "--context", "/fixture/temp/native-observer-aggregate-metadata/context.json",
              "--artifacts-metadata", "/fixture/temp/native-observer-aggregate-metadata/artifacts.json",
              "--github-output", "/fixture/github-output"]),
            (AGGREGATE, "Require all four same-attempt shards and exact 20 passing native methods", "native-1",
             [*common, "aggregate", *config, "--context", "/fixture/temp/native-observer-aggregate-metadata/context.json",
              "--archive-root", "/fixture/temp/native-observer-shard-download",
              "--artifacts-metadata", "/fixture/temp/native-observer-aggregate-metadata/artifacts.json",
              "--jobs-metadata", "/fixture/temp/native-observer-aggregate-metadata/jobs.json",
              "--consumer-run-metadata", "/fixture/temp/native-observer-aggregate-metadata/consumer-run.json",
              "--output-root", "/fixture/temp/native-observer-aggregate"]),
        ]
        for shard in SHARD_IDS:
            cases.append((SHARDS, "Run assigned actual native methods with zero skips", shard,
                          [*common, "run", *config, "--source-root", "/fixture/workspace/observer-source",
                           "--archive-root", "/fixture/temp/native-observer-download",
                           "--artifact-metadata", "/fixture/temp/native-observer-metadata/artifact.json",
                           "--run-metadata", "/fixture/temp/native-observer-metadata/run.json",
                           "--output-root", "/fixture/temp/native-observer-required", "--shard", shard,
                           "--context", "/fixture/temp/native-observer-metadata/context.json"]))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            recorder = root / "python"
            recorder.write_text("#!" + sys.executable + "\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n")
            recorder.chmod(0o700)
            env["PATH"] = str(root) + os.pathsep + env.get("PATH", "")
            for job_name, step_name, shard, expected in cases:
                with self.subTest(mode=expected[2], shard=shard):
                    selected = command(job(self.workflow, job_name), step_name)
                    result = subprocess.run(["/bin/bash", "--noprofile", "--norc", "-c", selected],
                                            env=dict(env, SHARD=shard), cwd=root, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(json.loads(result.stdout), expected)

    def test_frozen_context_is_retained_as_data_without_shell_evaluation(self):
        for selected_job, selected_step, folder in (
            (SHARDS, "Retain the immutable shard context without shell evaluation", "native-observer-metadata"),
            (AGGREGATE, "Retain the immutable aggregate context without shell evaluation", "native-observer-aggregate-metadata"),
        ):
            with self.subTest(job=selected_job), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binary = root / "bin"
                binary.mkdir()
                (binary / "python").symlink_to(sys.executable)
                if selected_job == SHARDS:
                    (root / folder).mkdir()
                value = {"literal": "$(touch should-not-exist); `touch also-absent`", "count": 4}
                result = subprocess.run(
                    ["/bin/bash", "--noprofile", "--norc", "-c", command(job(self.workflow, selected_job), selected_step)],
                    env=dict(os.environ, RUNNER_TEMP=str(root), CONTEXT_JSON=json.dumps(value),
                             PATH=str(binary) + os.pathsep + os.environ.get("PATH", "")),
                    cwd=root, capture_output=True, text=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                target = root / folder / "context.json"
                self.assertEqual(json.loads(target.read_text()), value)
                self.assertEqual(target.stat().st_mode & 0o777, 0o600)
                self.assertFalse((root / "should-not-exist").exists())
                self.assertFalse((root / "also-absent").exists())

    def test_aggregate_dependencies_fail_closed_for_every_non_success_result(self):
        selected = command(job(self.workflow, AGGREGATE), "Require complete immutable inputs for aggregation")
        for inputs in ("success", "failure", "cancelled", "skipped", ""):
            for shards in ("success", "failure", "cancelled", "skipped", ""):
                with self.subTest(inputs=inputs, shards=shards):
                    result = subprocess.run(["/bin/bash", "--noprofile", "--norc", "-c", selected],
                                            env=dict(os.environ, INPUTS_RESULT=inputs, SHARDS_RESULT=shards),
                                            capture_output=True, text=True)
                    self.assertEqual(result.returncode == 0, inputs == shards == "success")


if __name__ == "__main__":
    unittest.main()
