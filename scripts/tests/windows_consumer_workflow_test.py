"""Guard exact Windows consumer coverage without compiling or executing Rust.

Requires Python 3.11+ and Bash (Git for Windows on Windows). The native-platform
aggregate runs this guard; fixture subprocesses execute only its result check.
"""
from __future__ import annotations

import hashlib
import itertools
import os
from pathlib import Path
import re
import subprocess
import unittest

from native_bash_fixture import bash_executable

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/platform_native.yml"
STAGES = {'windows-consumer-compilation': ['Compile actual TestNetwork, integration and CLI test consumers'], 'windows-consumer-core': ['Exercise packed archive publication and retained readers on Windows', 'Exercise native evidence publication barriers on Windows'], 'windows-consumer-torii': ['Exercise Torii checkpoint custody and webhook persistence on NTFS']}
STAGE_NAMES = {'windows-consumer-compilation': 'Native Windows consumer compilation stage', 'windows-consumer-core': 'Native Windows Core publication stage', 'windows-consumer-torii': 'Native Windows Torii persistence stage'}
ARTIFACT_SUFFIXES = {'windows-consumer-compilation': 'compilation', 'windows-consumer-core': 'core', 'windows-consumer-torii': 'torii'}
STAGE_POLICY = ' windows-latest\n    timeout-minutes: 90\n    defaults:\n      run:\n        shell: bash\n    env:\n      CARGO_BUILD_JOBS: "1"\n      CARGO_PROFILE_DEV_DEBUG: "0"\n      CARGO_PROFILE_TEST_DEBUG: "0"\n'
SETUP_SHA256 = '367a3c9fc1f1e284872a2d5515e934b3e22835aaf521d023b78710d049a63532'
STEP_SHA256 = {'Compile actual TestNetwork, integration and CLI test consumers': '0b59b7693535c9e482032a963729b1922bd64cedc88b9d3928d5a7b677a71ccf', 'Exercise packed archive publication and retained readers on Windows': '1318b0e46960cdd00d83b82c06a104c3c52d3d80c2cb11f3f16e8ac6b1154edb', 'Exercise native evidence publication barriers on Windows': '6f74414b79f01491badb94a85f03d9a7be6e477d368e2e8be6f5d62550bf2103', 'Exercise Torii checkpoint custody and webhook persistence on NTFS': 'a022a834823699925061e0dfb679ef87b013f25935c0b1e4de688f6899be7392'}
UPLOAD = 'if: always()\n        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02\n        with:\n          name: platform-consumers-x86_64-pc-windows-msvc-${{ github.run_id }}\n          path: target/platform-consumers\n          if-no-files-found: error\n          retention-days: 14'
AGGREGATE_BODY = 'set -euo pipefail\nif [[ "${COMPILATION_RESULT}" != "success" || "${CORE_RESULT}" != "success" || "${TORII_RESULT}" != "success" ]]; then\n  printf \'Windows consumer stages must all succeed: compilation=%s core=%s torii=%s\\n\' "${COMPILATION_RESULT}" "${CORE_RESULT}" "${TORII_RESULT}"\n  exit 1\nfi\n'


def jobs(source):
    section = source.split("\njobs:\n", 1)[1]
    headers = list(re.finditer(r"^  ([a-z0-9_-]+):$", section, re.M))
    names = [match.group(1) for match in headers]
    assert len(names) == len(set(names)), "duplicate job"
    return {match.group(1): section[match.start():headers[index + 1].start()
            if index + 1 < len(headers) else len(section)]
            for index, match in enumerate(headers)}


def named_steps(job):
    headers = list(re.finditer(r"^      - name: (.+)$", job, re.M))
    names = [match.group(1) for match in headers]
    assert len(names) == len(set(names)), "duplicate named step"
    return {match.group(1): job[match.start():headers[index + 1].start()
            if index + 1 < len(headers) else len(job)]
            for index, match in enumerate(headers)}


def require_conservation(source):
    parsed = jobs(source)
    actual = {name for name in parsed if name.startswith("windows-consumer-")}
    assert actual == {*STAGES, "windows-consumer-check"}, "closed three-stage topology"
    for job_id, owned in STAGES.items():
        job = parsed[job_id]
        prefix, steps = job.split("    steps:\n", 1)
        assert prefix == f"  {job_id}:\n    name: {STAGE_NAMES[job_id]}\n    runs-on:" + STAGE_POLICY
        assert "continue-on-error" not in job
        first = steps.index("      - name: " + owned[0] + "\n")
        assert hashlib.sha256(steps[:first].encode()).hexdigest() == SETUP_SHA256
        named = named_steps(job)
        assert list(named) == ["Require the native Windows compiler", *owned,
                               "Retain native consumer compilation results"]
        for name in owned:
            assert hashlib.sha256(named[name].encode()).hexdigest() == STEP_SHA256[name]
        suffix = ARTIFACT_SUFFIXES[job_id]
        expected_upload = UPLOAD.replace(
            "platform-consumers-x86_64-pc-windows-msvc-${{ github.run_id }}",
            "platform-consumers-x86_64-pc-windows-msvc-${{ github.run_id }}-${{ github.run_attempt }}-" + suffix,
        )
        assert named["Retain native consumer compilation results"].split("\n", 1)[1].strip() == expected_upload
    aggregate = parsed["windows-consumer-check"]
    assert "    name: Native Windows consumer test compilation\n" in aggregate
    assert "    runs-on: windows-latest\n    timeout-minutes: 90\n    if: always()\n" in aggregate
    needs = aggregate.split("    needs:\n", 1)[1].split("    defaults:\n", 1)[0]
    assert needs == "".join("      - " + name + "\n" for name in STAGES)
    result_bindings = aggregate.split("        env:\n", 1)[1].split("        run: |\n", 1)[0]
    assert result_bindings == "".join(
        f"          {variable}: ${{{{ needs.{job_id}.result }}}}\n"
        for variable, job_id in zip(("COMPILATION_RESULT", "CORE_RESULT", "TORII_RESULT"), STAGES)
    )
    assert '      - "scripts/tests/windows_consumer_workflow_test.py"\n' in source
    assert "        run: python -m unittest discover -s scripts/tests -p windows_consumer_workflow_test.py -v\n" in aggregate
    assert "continue-on-error" not in aggregate and "\n        if:" not in aggregate
    body = aggregate.split("        run: |\n", 1)[1]
    body = "".join(line[10:] if line.startswith("          ") else line for line in body.splitlines(keepends=True))
    assert body == AGGREGATE_BODY
    return body


class WindowsConsumerWorkflowTests(unittest.TestCase):
    def test_exact_commands_setup_policy_and_closed_aggregate(self):
        require_conservation(WORKFLOW.read_text())

    def test_scope_guard_refuses_policy_coverage_and_dependency_mutations(self):
        source = WORKFLOW.read_text()
        parsed = jobs(source)
        cases = []
        for job_id in STAGES:
            cases.append(source.replace(parsed[job_id], "", 1))
        cases.append(source + parsed["windows-consumer-core"])
        cases.append(source + parsed["windows-consumer-core"].replace("windows-consumer-core:", "windows-consumer-extra:", 1))
        core = parsed["windows-consumer-core"]
        packed = named_steps(core)[STAGES["windows-consumer-core"][0]]
        cases.append(source.replace(core, core.replace(packed, packed + packed, 1), 1))
        for old, new in (
            ('    timeout-minutes: 90', '    timeout-minutes: 120'),
            ('    runs-on: windows-latest', '    runs-on: ubuntu-latest'),
            ('      CARGO_BUILD_JOBS: "1"', '      CARGO_BUILD_JOBS: "2"'),
            ('      CARGO_PROFILE_TEST_DEBUG: "0"', '      CARGO_PROFILE_TEST_DEBUG: "1"'),
            ('          toolchain: 1.99.1', '          toolchain: stable'),
            ('          persist-credentials: false', '          persist-credentials: true'),
            ('    timeout-minutes: 90\n', '    timeout-minutes: 90\n    if: false\n'),
            ('    timeout-minutes: 90\n', '    timeout-minutes: 90\n    needs: windows-consumer-compilation\n'),
            ('query::native_context_archive::packed::tests::', 'query::native_context_archive::other::tests::'),
            ('-- --nocapture', '-- --nocapture --test-threads=1'),
            ('--lib query::', '--features substituted --lib query::'),
            ('        if: always()', '        if: success()'),
            ('          retention-days: 14', '          retention-days: 1'),
            ('-${{ github.run_attempt }}-core', '-compilation'),
        ):
            assert old in core
            cases.append(source.replace(core, core.replace(old, new, 1), 1))
        torii = parsed["windows-consumer-torii"]
        cases.append(source.replace(torii, torii.replace(
            'native_checkpoint_refuses_aliased_lock_before_reading_absent_checkpoint', 'different_test', 1), 1))
        aggregate = parsed["windows-consumer-check"]
        for old, new in (
            ('    if: always()', '    if: success()'),
            ('      - windows-consumer-torii\n', ''),
            ('needs.windows-consumer-torii.result', 'needs.windows-consumer-core.result'),
            ('          COMPILATION_RESULT: ${{ needs.windows-consumer-compilation.result }}\n',
             '          COMPILATION_RESULT: ${{ needs.windows-consumer-compilation.result }}\n          COMPILATION_RESULT: success\n'),
            ('${TORII_RESULT}" != "success"', '${TORII_RESULT}" == "failure"'),
            ('        run: python -m unittest discover', '        run: echo omitted unittest discover'),
        ):
            assert old in aggregate
            cases.append(source.replace(aggregate, aggregate.replace(old, new, 1), 1))
        cases.append(source.replace('      - "scripts/tests/windows_consumer_workflow_test.py"\n', '', 1))
        for number, mutated in enumerate(cases):
            with self.subTest(mutation=number), self.assertRaises((AssertionError, KeyError, ValueError)):
                require_conservation(mutated)
        self.assertEqual(len(cases), 28)

    def test_aggregate_requires_all_three_exact_successes(self):
        body = require_conservation(WORKFLOW.read_text())
        names = ("COMPILATION_RESULT", "CORE_RESULT", "TORII_RESULT")
        outcomes = ("success", "failure", "cancelled", "skipped", "", None)
        for values in itertools.product(outcomes, repeat=3):
            env = {key: value for key, value in os.environ.items() if key not in names}
            env.update({name: value for name, value in zip(names, values) if value is not None})
            with self.subTest(values=values):
                result = subprocess.run([bash_executable(), "-c", body], env=env,
                                        capture_output=True, text=True, timeout=15)
                self.assertEqual(result.returncode == 0, values == ("success",) * 3)


if __name__ == "__main__":
    unittest.main()
