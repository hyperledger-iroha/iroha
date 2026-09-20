"""Standard-library contract tests for the isolated test-only phase harness."""
import sys
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('fixed_preflight_driver', ROOT/'pytests/scripts/run_scaling_preflight.py')
m = importlib.util.module_from_spec(SPEC)
driver_raw = Path(SPEC.origin).read_bytes()
if not 0 < len(driver_raw) <= 8*1024*1024:
    raise ValueError('phase driver source size')
exec(compile(driver_raw,SPEC.origin,'exec',dont_inherit=True),m.__dict__)
if Path(SPEC.origin).read_bytes() != driver_raw:
    raise ValueError('phase driver changed during helper import')


class HarnessContract(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
    def tearDown(self):
        self.temp.cleanup()
    def test_absolute_inputs_reject_relative_and_parent_traversal(self):
        self.assertEqual(m._absolute(str(self.root)), self.root)
        for value in ('relative', str(self.root/'..'/'other')):
            with self.subTest(value=value), self.assertRaises(ValueError):m._absolute(value)
    def test_exact_registry_rejects_missing_duplicate_and_extra_members(self):
        original = m._source_paths(ROOT)
        self.assertEqual(len(original), 58)
        source = self.root/'scripts/nexus/scaling_cli_bootstrap.py'
        source.parent.mkdir(parents=True)
        for values in ((), original[:-1], original+(original[0],), original+('scripts/extra.py',)):
            source.write_text('PYTHON_SOURCE_FILES = '+repr(values)+'\n')
            with self.subTest(size=len(values)), self.assertRaises(ValueError):m._source_paths(self.root)
    def test_copy_is_bound_exclusive_and_owner_only(self):
        source=self.root/'input';source.write_bytes(b'original')
        destination=self.root/'private/nested/input';bound=m._sha(source)
        m._copy_file(source,destination,bound)
        self.assertEqual(destination.read_bytes(),b'original')
        self.assertEqual(destination.stat().st_mode&0o777,0o600)
        self.assertEqual(destination.parent.stat().st_mode&0o777,0o700)
        with self.assertRaises(FileExistsError):m._copy_file(source,destination,bound)
        source.write_bytes(b'changed')
        with self.assertRaises(ValueError):m._copy_file(source,self.root/'new',bound)
        self.assertFalse((self.root/'new').exists())
    def test_nested_node_ids_count_subtests_once(self):
        class Sample(unittest.TestCase):
            def test_value(self):
                for value in range(3):
                    with self.subTest(value=value):self.assertLess(value,3)
        suite=unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(Sample)])
        expected=list(m._ids(suite));self.assertEqual(len(expected),1)
        result=unittest.TextTestRunner(stream=io.StringIO(),resultclass=m._Result).run(suite)
        self.assertEqual(result.node_ids,expected);self.assertEqual(result.testsRun,1)
        self.assertEqual(result.subtest_observations,3)
    def test_every_phase_rejects_zero_selection_before_case_import(self):
        replica=self.root/'repo';replica.mkdir()
        names=(*m._source_paths(ROOT),'scripts/nexus/scaling_release_provisioning.py',
               'scripts/nexus/scaling_preflight_archive.py',
               'pytests/scripts/run_scaling_preflight.py')
        for name in names:
            target=replica/name;target.parent.mkdir(parents=True,exist_ok=True)
            target.write_bytes((ROOT/name).read_bytes())
        cases=Path('pytests/scripts/scaling_preflight')
        shutil.copytree(ROOT/cases,replica/cases)
        node_path=replica/cases/'phase_nodes.json'
        original=json.loads(node_path.read_bytes())
        dependencies=self.root/'dependencies';dependencies.mkdir()
        for phase in m.PHASE_COUNTS:
            empty={**original,phase:[]};node_path.write_text(json.dumps(empty))
            argv=['driver','--phase',phase,'--repository-root',str(replica),
                  '--dependency-root',str(dependencies),'--work-root',str(self.root/(phase+'-work')),
                  '--result',str(self.root/(phase+'-result.json'))]
            with (self.subTest(phase=phase),patch.object(m,'__file__',str(replica/'pytests/scripts/run_scaling_preflight.py')),
                 patch.object(sys,'argv',argv),patch.object(m.importlib,'import_module',side_effect=AssertionError('case import before selection'))):
                with self.assertRaisesRegex(ValueError,'phase selection'):m.main()
            self.assertFalse((self.root/(phase+'-result.json')).exists())


if __name__ == '__main__':
    if not (sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode):
        raise SystemExit('harness tests require the actual -I -B -S flags')
    sys.pycache_prefix = '/__fixed_scaling_preflight_unused_pycache__'
    def audit(event,arguments):
        if event in m.FORBIDDEN_EVENTS:raise AssertionError('forbidden process operation: '+event)
    sys.addaudithook(audit)
    unittest.main(verbosity=2)
