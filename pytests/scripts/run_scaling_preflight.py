#!/usr/bin/env python3
"""Run one fixed collector test phase in an already isolated interpreter.

The release process owner invokes this file separately for every phase with
-I -B -S and parent-selected admitted dependency files. This standard-library
harness never launches a process. These file/service tests do not qualify a
native build, an archived interpreter, a host, or a performance experiment.
"""
import sys

import argparse
import ast
import hashlib
import importlib
import json
import os
from pathlib import Path
import time
import unittest

def _phase_protocol(root):
    """Read the sole source-bound count literal without admitting another module."""
    path = root/'scripts/nexus/scaling_preflight_archive.py'
    raw = path.read_bytes()
    if not 0 < len(raw) <= 8*1024*1024:
        raise ValueError('preflight protocol source size')
    tree = ast.parse(raw)
    rows = [node.value for node in tree.body if isinstance(node, ast.Assign)
        and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
        and node.targets[0].id == 'PHASE_COUNTS']
    if len(rows) != 1:
        raise ValueError('preflight phase protocol source missing')
    counts = ast.literal_eval(rows[0])
    if (type(counts) is not dict or not counts or any(type(name) is not str
            or type(count) is not int or count <= 0 for name,count in counts.items())
            or path.read_bytes() != raw):
        raise ValueError('preflight phase protocol source changed')
    return raw, counts


_protocol_raw, PHASE_COUNTS = _phase_protocol(Path(__file__).resolve().parents[2])
FORBIDDEN_EVENTS = frozenset(('subprocess.Popen', 'os.system', 'os.posix_spawn',
    'os.fork', 'os.forkpty', 'os.kill', 'os.killpg', 'os.exec', 'os.spawn'))


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _absolute(value: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path != path.resolve():
        raise ValueError('preflight paths must be absolute and canonical')
    return path


def _source_paths(root: Path) -> tuple[str, ...]:
    tree = ast.parse((root/'scripts/nexus/scaling_cli_bootstrap.py').read_bytes())
    values = [node.value for node in tree.body if isinstance(node, ast.Assign)
              and any(isinstance(target, ast.Name) and target.id == 'PYTHON_SOURCE_FILES'
                      for target in node.targets)]
    if len(values) != 1:
        raise ValueError('missing canonical child source registry')
    result = ast.literal_eval(values[0])
    if (type(result) is not tuple or len(result) != 58
            or result != tuple(sorted(set(result)))
            or any(type(name) is not str or not name.startswith('scripts/')
                   or '..' in Path(name).parts for name in result)):
        raise ValueError('unexpected fixed collector source registry')
    return result


def _copy_file(source: Path, target: Path, bound: str) -> None:
    if _sha(source) != bound:
        raise ValueError('preflight input changed before copy')
    parents = []
    parent = target.parent
    while not parent.exists():
        parents.append(parent)
        parent = parent.parent
    for parent in reversed(parents):
        parent.mkdir(mode=0o700)
    with target.open('xb') as stream:
        stream.write(source.read_bytes())
    target.chmod(0o600)
    if _sha(target) != bound or _sha(source) != bound:
        raise ValueError('preflight input changed while copying')


def _ids(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from _ids(item)
        else:
            yield item.id()


class _Result(unittest.TextTestResult):
    """Count original test nodes once; subtests remain assertions of that node."""
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.node_ids = []
        self.subtest_observations = 0

    def startTest(self, test):
        self.node_ids.append(test.id())
        super().startTest(test)

    def addSubTest(self, test, subtest, err):
        self.subtest_observations += 1
        super().addSubTest(test, subtest, err)


def main() -> int:
    if not (sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode):
        raise SystemExit('fixed scaling preflight requires the actual -I -B -S flags')
    # Importing the helper surface leaves its host untouched. Executable phases
    # set their own cache scope before importing any test or production module.
    sys.pycache_prefix = '/__fixed_scaling_preflight_unused_pycache__'
    protocol_raw, phase_counts = _phase_protocol(Path(__file__).resolve().parents[2])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase', required=True, choices=tuple(phase_counts))
    parser.add_argument('--repository-root', required=True, type=_absolute)
    parser.add_argument('--dependency-root', required=True, type=_absolute)
    parser.add_argument('--work-root', required=True, type=_absolute)
    parser.add_argument('--result', required=True, type=_absolute)
    args = parser.parse_args()
    root = args.repository_root
    if root != Path(__file__).resolve().parents[2]:
        raise ValueError('test driver must belong to the selected repository')
    if not args.dependency_root.is_dir():
        raise ValueError('parent-selected dependency root is missing')
    if args.work_root.exists() or args.result.exists():
        raise ValueError('work and result paths must be fresh')
    args.work_root.mkdir(mode=0o700)
    sys.pycache_prefix = str(args.work_root/'unused-pycache')
    cases = root/'pytests/scripts/scaling_preflight'
    paths = _source_paths(root)
    inputs = {name: _sha(root/name) for name in paths}
    for path in sorted(cases.rglob('*')):
        if path.is_file() and (path.suffix in ('.py', '.json')):
            inputs[path.relative_to(root).as_posix()] = _sha(path)
    inputs['pytests/scripts/run_scaling_preflight.py'] = _sha(Path(__file__))
    # The provisioning owner is parent-only and must never enter child closure.
    parent_name = 'scripts/nexus/scaling_release_provisioning.py'
    inputs[parent_name] = _sha(root/parent_name)
    inputs['scripts/nexus/scaling_preflight_archive.py'] = hashlib.sha256(protocol_raw).hexdigest()
    copied = args.work_root/'candidate'
    copied.mkdir(mode=0o700)
    for name in paths:
        _copy_file(root/name, copied/name, inputs[name])
    for name in ('plan.json', 'budget.json', 'bounds.json'):
        source = cases/'fixtures'/name
        _copy_file(source, args.work_root/'fixtures'/name, _sha(source))
    expected = json.loads((cases/'phase_nodes.json').read_bytes())[args.phase]
    if (type(expected) is not list or len(expected) != phase_counts[args.phase]
            or expected != sorted(set(expected))):
        raise ValueError('empty, duplicate or unexpected fixed phase selection')
    attempts = []
    def audit(event, arguments):
        if event in FORBIDDEN_EVENTS:
            attempts.append(event)
            raise AssertionError('preflight attempted forbidden process operation: '+event)
    sys.addaudithook(audit)
    sys.path[:0] = [str(cases), str(copied/'scripts'), str(copied/'scripts/nexus'),
                   str(root/'scripts'), str(root/'scripts/nexus')]
    from preflight_context import PreflightContext, install_context
    install_context(PreflightContext(root, args.dependency_root, args.work_root))
    record = {'phase': args.phase, 'isolated': bool(sys.flags.isolated),
        'no_site': bool(sys.flags.no_site), 'no_bytecode': bool(sys.flags.dont_write_bytecode),
        'source_registry_count': len(paths), 'expected_node_ids': expected,
        'inputs_before': inputs, 'external_native_processes': False,
        'qualification': 'pure/file tests with explicitly declared host/framework/process seams'}
    started = time.monotonic_ns()
    success = False
    try:
        module = importlib.import_module(args.phase+'_cases')
        suite = unittest.defaultTestLoader.loadTestsFromModule(module)
        selected = sorted(_ids(suite))
        if selected != expected:
            raise ValueError('collected phase node IDs differ from exact selection')
        result = unittest.TextTestRunner(verbosity=2, resultclass=_Result).run(suite)
        record.update(tests_run=result.testsRun, node_ids=sorted(result.node_ids),
            failures=len(result.failures), errors=len(result.errors), skipped=len(result.skipped),
            subtest_observations=result.subtest_observations)
        success = (result.wasSuccessful() and not result.skipped
                   and sorted(result.node_ids) == expected)
        if args.phase == 'bootstrap':
            record.update(actual_private_blake3_import=module.ACTUAL_IMPORT,
                          actual_bootstrap_composition=module.ACTUAL_BOOTSTRAP)
            success = success and module.ACTUAL_IMPORT and module.ACTUAL_BOOTSTRAP
    except Exception as error:
        record['harness_error_type'] = type(error).__name__
        raise
    finally:
        after = {name: _sha(root/name) for name in inputs}
        copied_after = {name: _sha(copied/name) for name in paths}
        unchanged = inputs == after and all(copied_after[name] == inputs[name] for name in paths)
        record.update(inputs_after=after, copied_sources_after=copied_after,
            inputs_unchanged=unchanged, forbidden_process_attempts=attempts,
            elapsed_ns=time.monotonic_ns()-started,
            passed=bool(success and unchanged and not attempts))
        with args.result.open('x') as stream:
            json.dump(record, stream, sort_keys=True, indent=2)
            stream.write('\n')
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
