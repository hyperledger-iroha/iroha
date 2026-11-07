"""Offline guards for fast classification and mandatory unchanged helper coverage.

Python 3.11 and Bash required; only workflow text and synthetic result strings
are evaluated. No Cargo, compiler, native test or network operation runs here.
"""
import hashlib
import os
from pathlib import Path
import re
import subprocess
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[2]
BODIES = {
    ('rust_changes', 'Verify exhaustive Rust lane ownership'): '2b0054d333280f997ce670e3555ce798fa4a0e43a5928f79f141404d35654502',
    ('rust_helper_controls', 'Exercise the complete original helper test inventory'): '0bf44fa13f5bfe565c48bc7f710d08e899f32899f9a87857b00849f1452fa27d',
    ('rust_source_guards', 'Run the complete original source guards'): '3a5ae2fe8e046ff906d9793c0edcea93bca99d2a766b561d3b372fd3c13748dd',
}
SETUP_SHA256 = '8593deda99de293ea4e6962c758fb4184bf75cc7fe2e4e448f81538e9ceda400'
ORIGINAL_BODY_SHA256 = '545ab314e9e36f2481ad03eff19c4e9cb094ba19d256faef45f449417ef4a4f6'


def job(workflow, name):
    found = re.findall(r'(?ms)^  ' + re.escape(name) + r':\n(.*?)(?=^  [a-z_]+:|\Z)', workflow)
    if len(found) != 1:
        raise AssertionError('exactly one job required: ' + name)
    return found[0]


def script(body, step):
    found = re.findall(r'(?ms)^      - name: ' + re.escape(step) + r'\n        run: \|\n(.*?)(?=^      - |\Z)', body)
    if len(found) != 1:
        raise AssertionError('exactly one unchanged run body required: ' + step)
    return found[0]


def digest(text):
    return hashlib.sha256(text.encode()).hexdigest()


class RequiredHelperWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.workflow = (ROOT / '.github/workflows/pr.yml').read_text()

    def assert_contract(self, workflow):
        bodies = []
        for (name, step), expected in BODIES.items():
            body = job(workflow, name)
            self.assertIn('    runs-on: ubuntu-latest\n', body)
            self.assertIn('    timeout-minutes: 45\n', body)
            self.assertNotRegex(body, r'(?m)^    (?:if|needs|continue-on-error):')
            setup = body.split('    steps:\n', 1)[1].split('      - name: Install pinned router-test dependency\n', 1)
            self.assertEqual(len(setup), 2)
            setup = setup[0] + '      - name: Install pinned router-test dependency\n' + setup[1].split('      - name:', 1)[0]
            self.assertEqual(digest(setup), SETUP_SHA256)
            run = script(body, step)
            self.assertEqual(digest(run), expected)
            self.assertNotIn('continue-on-error:', body)
            bodies.append(run)
        self.assertEqual(digest(''.join(bodies)), ORIGINAL_BODY_SHA256)
        classifier = job(workflow, 'rust_changes')
        self.assertNotIn('rust_helper_controls', classifier)
        self.assertNotIn('rust_source_guards', classifier)
        self.assertIn('      - name: Classify the PR reverse-dependency closure\n', classifier)
        self.assertIn('      - name: Upload Rust classification evidence\n', classifier)
        helper = job(workflow, 'rust_helper_controls')
        self.assertIn('python3 -B -m unittest discover -s scripts/tests -p rust_helper_workflow_test.py -v', helper)
        self.assertEqual(workflow.count('--deselect=scripts/tests/zk_resource_harness_test.py::NativeRunTests'), 1)
        self.assertIn('--deselect=scripts/tests/zk_resource_harness_test.py::NativeRunTests', helper)
        aggregate = job(workflow, 'rust_required')
        self.assertIn('    if: always()\n', aggregate)
        needs = re.search(r'(?m)^    needs: \[([^\n]+)\]', aggregate)
        self.assertIsNotNone(needs)
        for name, variable in [('rust_helper_controls', 'HELPER_CONTROLS_RESULT'), ('rust_source_guards', 'SOURCE_GUARDS_RESULT')]:
            self.assertIn(name, needs[1].split(', '))
            self.assertIn(variable + ': ${{ needs.' + name + '.result }}', aggregate)
            self.assertIn('test "$' + variable + '" = success', aggregate)

    def test_original_commands_setup_deadlines_and_required_topology_are_preserved(self):
        self.assert_contract(self.workflow)

    def test_dropped_control_changed_deadline_or_conditional_helpers_are_rejected(self):
        mutations = {
            'dropped_helper': ('            scripts/tests/check_zk_resource_contract_test.py \\\n', ''),
            'dropped_source_guard': ('          python3 -I -S scripts/check_zk_resource_contract.py\n', ''),
            'conditional_helpers': ('  rust_helper_controls:\n', '  rust_helper_controls:\n    if: false\n'),
            'conditional_guards': ('  rust_source_guards:\n', '  rust_source_guards:\n    if: false\n'),
            'classifier_blocked_by_helpers': ('  rust_changes:\n', '  rust_changes:\n    needs: rust_helper_controls\n'),
            'longer_deadline': ('    timeout-minutes: 45\n', '    timeout-minutes: 46\n'),
            'missing_required_helper': ('needs: [rust_helper_controls, rust_source_guards, rust_changes,', 'needs: [rust_source_guards, rust_changes,'),
            'missing_result_guard': ('          test "$SOURCE_GUARDS_RESULT" = success\n', ''),
            'native_coverage_skipped': ('--deselect=scripts/tests/zk_resource_harness_test.py::NativeRunTests', '--deselect=scripts/tests/zk_resource_harness_test.py'),
        }
        for name, (old, new) in mutations.items():
            with self.subTest(mutation=name):
                self.assertIn(old, self.workflow)
                with self.assertRaises(AssertionError):
                    self.assert_contract(self.workflow.replace(old, new))

    def test_required_aggregate_rejects_every_non_success_or_missing_helper_result(self):
        aggregate = job(self.workflow, 'rust_required')
        shell = textwrap.dedent(aggregate.split('        run: |\n', 1)[1])
        environment = {'PATH': os.environ['PATH'], 'CLASSIFIER_RESULT': 'success', 'NATIVE_HARNESS_RESULT': 'success',
                       'HELPER_CONTROLS_RESULT': 'success', 'SOURCE_GUARDS_RESULT': 'success'}
        for prefix in ('BINARY_FREE', 'NETWORK', 'BUILD', 'CONSISTENCY', 'KOTODAMA', 'PYTESTS', 'PARLIAMENT'):
            environment[prefix + '_SELECTED'] = 'false'
            environment[prefix + '_RESULT'] = 'skipped'
        result = subprocess.run(['bash', '-c', shell], env=environment, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for variable in ('HELPER_CONTROLS_RESULT', 'SOURCE_GUARDS_RESULT'):
            for outcome in ('failure', 'cancelled', 'skipped', 'queued', 'in_progress', '', None):
                changed = dict(environment)
                if outcome is None:
                    del changed[variable]
                else:
                    changed[variable] = outcome
                with self.subTest(variable=variable, outcome=outcome):
                    result = subprocess.run(['bash', '-c', shell], env=changed, capture_output=True)
                    self.assertNotEqual(result.returncode, 0)


if __name__ == '__main__':
    unittest.main()
