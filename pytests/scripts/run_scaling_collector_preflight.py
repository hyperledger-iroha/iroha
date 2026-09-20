"""One source-bound collector suite in an original protected parent process.

The parent alone owns scheduling and the original deadline. This child imports
only the explicitly admitted test package closure and BLAKE3; it cannot declare
the complete release preflight successful from one suite's report.
"""
import sys
if not (sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode):
    raise SystemExit('collector preflight requires the actual -I -B -S flags')
sys.pycache_prefix = '/__fixed_scaling_collector_unused_pycache__'
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile


def absolute(value):
    path = Path(value)
    if not path.is_absolute() or path != path.resolve():
        raise ValueError('collector preflight paths must be absolute and canonical')
    return path


def load_source(name, path):
    """Execute the source buffer itself, never an adjacent bytecode cache."""
    if name in sys.modules: raise ValueError('preflight source module already imported')
    raw = path.read_bytes()
    if not 0 < len(raw) <= 8*1024*1024: raise ValueError('preflight source size')
    spec = importlib.util.spec_from_file_location(name,path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(raw,str(path),'exec',dont_inherit=True),module.__dict__)
    if path.read_bytes() != raw: raise ValueError('preflight source changed during import')
    return module


def node_digest(node_id):
    # Parametrized IDs may contain multi-megabyte negative-test inputs. Commit
    # their exact UTF-8 bytes without retaining another unbounded report copy.
    return hashlib.sha256(node_id.encode('utf-8')).hexdigest()


class ExactPytestOutcomes:
    """Observe collection and all three actual phases of every required node."""
    def __init__(self, expected):
        self.expected = tuple(expected)
        self.collected, self.reports, self.deselected = [], {}, []
        self.failures = self.errors = self.skipped = self.subtests = 0

    def pytest_collection_modifyitems(self, session, config, items):
        self.collected = sorted(node_digest(item.nodeid) for item in items)
        if self.collected != list(self.expected):
            raise ValueError('collector preflight selection differs from exact source inventory')

    def pytest_deselected(self, items):
        self.deselected.extend(node_digest(item.nodeid) for item in items)

    def pytest_collectreport(self, report):
        if report.failed: self.errors += 1
        if report.skipped: self.skipped += 1

    def pytest_runtest_logreport(self, report):
        # A subtest is an additional observation, never another required node.
        # Its failure/skip still rejects the original containing test outcome.
        subtest = hasattr(report,'context')
        if subtest: self.subtests += 1
        if report.failed:
            if report.when == 'call': self.failures += 1
            else: self.errors += 1
        if report.skipped or hasattr(report,'wasxfail'): self.skipped += 1
        if subtest: return
        node = node_digest(report.nodeid)
        if node not in self.expected or report.when not in ('setup','call','teardown'):
            self.errors += 1
            return
        phases = self.reports.setdefault(node,{})
        if report.when in phases: self.errors += 1
        phases[report.when] = report.outcome

    def result(self, exit_code):
        outcomes = sorted(name for name,phases in self.reports.items()
            if phases == {'setup':'passed','call':'passed','teardown':'passed'})
        good = (type(exit_code) is int and exit_code == 0 and
            self.collected == list(self.expected) == outcomes and not self.deselected
            and self.failures == self.errors == self.skipped == 0)
        return dict(tests_run=len(self.reports), node_sha256s=sorted(self.reports),
            collected_node_sha256s=self.collected,outcome_node_sha256s=outcomes,
            failures=self.failures,errors=self.errors,skipped=self.skipped,
            subtest_observations=self.subtests,passed=good)


def execute_suite(pytest, root, work, name, expected):
    observer = ExactPytestOutcomes(expected)
    work.mkdir(mode=0o700,parents=True,exist_ok=True)
    config = work/'pytest.ini'
    with config.open('xb') as stream: stream.write(b'[pytest]\n')
    config.chmod(0o600)
    status = pytest.main(['-q','--disable-plugin-autoload','-p','no:cacheprovider','--noconftest','-c',str(config),'--rootdir='+str(root),
        '--confcutdir='+str(root),'--basetemp='+str(work/'pytest'),str(root/name)],
        plugins=[observer])
    return observer.result(int(status))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--suite',required=True)
    for name in ('repository-root','work-root','result',
                 'dependency-source','dependency-bundle','dependency-inventory',
                 'blake3-dependency-source','blake3-dependency-bundle','blake3-dependency-inventory'):
        parser.add_argument('--'+name,type=absolute,required=True)
    parser.add_argument('--dependency-inventory-sha256',required=True)
    parser.add_argument('--blake3-dependency-inventory-sha256',required=True)
    args = parser.parse_args()
    root = args.repository_root
    if root != Path(__file__).resolve().parents[2]:
        raise ValueError('collector preflight source root mismatch')
    if args.work_root.exists() or args.result.exists():
        raise ValueError('collector preflight output paths must be fresh')
    args.work_root.mkdir(mode=0o700)
    sys.pycache_prefix = str(args.work_root/'unused-pycache')
    sys.path[:0] = [str(root/'scripts'),str(root/'scripts/nexus'),str(root/'pytests/scripts')]
    contract = load_source('scaling_preflight_archive',root/'scripts/nexus/scaling_preflight_archive.py')
    raw = (root/contract.INVENTORY_PATH).read_bytes()
    inventory = contract.decode_inventory(raw)
    expected = inventory['pytest_suites'].get(args.suite)
    contract.require(expected is not None, 'unknown collector preflight suite')
    def inputs():
        return {name:hashlib.sha256((root/name).read_bytes()).hexdigest()
            for name in inventory['sources']}
    before = inputs()
    contract.require(before == inventory['sources'], 'collector preflight source binding failed')
    # Prevent inherited pytest options and plugin entrypoint discovery from
    # changing the one exact selection. -I/-S already ignore Python path knobs.
    os.environ.pop('PYTEST_ADDOPTS',None)
    os.environ.pop('PYTEST_PLUGINS',None)
    os.environ['PYTEST_DISABLE_PLUGIN_AUTOLOAD'] = '1'
    tempfile.tempdir = str(args.work_root)
    dependencies = load_source('scaling_cli_bootstrap',root/'scripts/nexus/scaling_cli_bootstrap.py')
    package = blake3 = None
    record = dict(suite=args.suite,inventory_sha256=hashlib.sha256(raw).hexdigest(),
        isolated=bool(sys.flags.isolated),no_site=bool(sys.flags.no_site),
        no_bytecode=bool(sys.flags.dont_write_bytecode),inputs_before=before,passed=False)
    try:
        paths = dependencies.PythonDependencyPaths(args.dependency_source,args.dependency_bundle,
            args.dependency_inventory,args.dependency_inventory_sha256)
        package = dependencies.PythonTestDependencies.admit(paths)
        pytest = package.load()
        blake3 = dependencies.PythonDependencies.admit(dependencies.PythonDependencyPaths(
            args.blake3_dependency_source,args.blake3_dependency_bundle,
            args.blake3_dependency_inventory,args.blake3_dependency_inventory_sha256))
        blake3.load()
        package.verify(); blake3.verify()
        record.update(execute_suite(pytest,root,args.work_root,args.suite,expected))
        package.verify(); blake3.verify()
    finally:
        try:
            after = inputs()
            unchanged = before == after and (root/contract.INVENTORY_PATH).read_bytes() == raw
            record.update(inputs_after=after,inputs_unchanged=unchanged,
                passed=bool(record['passed'] and unchanged))
        finally:
            try:
                if blake3 is not None: blake3.close()
            finally:
                if package is not None: package.close()
        payload = contract.canonical(record)
        contract.require(len(payload) <= contract.MAX_RESULT_BYTES)
        with args.result.open('xb') as stream: stream.write(payload)
        args.result.chmod(0o600)
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
