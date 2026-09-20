"""Isolated admission and custody tests for the fixed pure-Python pytest closure."""
from __future__ import annotations

import base64
import contextlib
import csv
from dataclasses import replace
import fcntl
import hashlib
import importlib
import importlib.util
import io
import marshal
import os
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import types
import unittest

from preflight_context import current_context

_CONTEXT = current_context()
import scaling_cli_bootstrap as m


class TestDependencies(unittest.TestCase):
    """Exercise actual staged files and imports without child processes."""

    def setUp(self):
        self.assertFalse(any(name.split('.')[0] in m._TEST_IMPORT_ROOTS
                             for name in sys.modules))
        m._exact_test_package(_CONTEXT.dependency_root)
        self.temp = tempfile.TemporaryDirectory(dir=_CONTEXT.work_root)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / 'source'
        shutil.copytree(_CONTEXT.dependency_root, self.source)
        self.owners = []
        self.foreign_modules = []
        self.replaced_descriptors = []

    def tearDown(self):
        # Restore only explicit test injections and remove only exact objects
        # imported by each test's owner. Never clear a namespace or sys.modules.
        for name, injected, previous in reversed(self.foreign_modules):
            if sys.modules.get(name) is injected:
                if previous is None:
                    del sys.modules[name]
                else:
                    sys.modules[name] = previous
        for owner in reversed(self.owners):
            pins = {name: module for name, module, _ in owner._module_pins}
            for alias, target in (('py.error', '_pytest._py.error'),
                                  ('py.path', '_pytest._py.path')):
                if target in pins and sys.modules.get(alias) is pins[target]:
                    del sys.modules[alias]
            for name, module, _ in reversed(owner._module_pins):
                if sys.modules.get(name) is module:
                    del sys.modules[name]
            owner.close()
        for descriptor, pin in self.replaced_descriptors:
            try:
                if m._descriptor_pin(descriptor) == pin:
                    os.close(descriptor)
            except OSError:
                pass
        for path in self.root.rglob('*'):
            if not path.is_symlink() and path.is_dir():
                path.chmod(0o700)
        self.temp.cleanup()

    def owner(self):
        owner = m.PythonTestDependencies.provision(
            self.source, self.root / 'bundle', self.root / 'inventory.json')
        self.owners.append(owner)
        return owner

    def inject(self, name, module):
        self.foreign_modules.append((name, module, sys.modules.get(name)))
        sys.modules[name] = module

    def rewrite(self, path, data):
        path.chmod(0o600)
        path.write_bytes(data)

    def refresh_record(self, relative, data):
        """Forge a consistent local RECORD; fixed selected-content roots still win."""
        path = self.source / relative
        self.rewrite(path, data)
        record = self.source / 'pytest-9.0.3.dist-info/RECORD'
        rows = list(csv.reader(io.StringIO(record.read_text(), newline='')))
        changed = False
        for row in rows:
            if row[0] == relative:
                row[1:] = [
                    'sha256=' + base64.urlsafe_b64encode(
                        hashlib.sha256(data).digest()).rstrip(b'=').decode('ascii'),
                    str(len(data)),
                ]
                changed = True
        self.assertTrue(changed)
        rendered = io.StringIO(newline='')
        csv.writer(rendered, lineterminator='\n').writerows(rows)
        self.rewrite(record, rendered.getvalue().encode())

    def assert_admission_rejected(self):
        with self.assertRaises((m.ScalingBootstrapError, m.bundle_contract.CacheCopyError,
                                FileNotFoundError)):
            self.owner()

    def assert_descriptor_rejected_and_preserved(self, owner, descriptor):
        pin = m._descriptor_pin(descriptor)
        self.replaced_descriptors.append((descriptor, pin))
        with self.assertRaises(m.ScalingBootstrapError):
            owner.validate()
        owner.close()
        self.assertEqual(m._descriptor_pin(descriptor), pin)

    def test_actual_private_import_and_one_assertion(self):
        owner = self.owner()
        original_paths = tuple(sys.path)
        pytest = owner.load()
        self.assertEqual(pytest.__version__, '9.0.3')
        self.assertEqual(tuple(sys.path), original_paths)
        self.assertNotIn(str(owner.paths.bundle_root), sys.path)
        self.assertNotIn(str(owner.paths.source_root), sys.path)
        for name in ('pygments.lexers', 'pygments.formatters'):
            module = importlib.import_module(name)
            self.assertIs(type(module), module.__dict__['_automodule'])
            self.assertEqual(type(module).__bases__, (types.ModuleType,))
            self.assertEqual(Path(module.__file__), owner.paths.bundle_root
                             / name.replace('.', '/') / '__init__.py')
        self.assertIs(sys.modules['py.error'], sys.modules['_pytest._py.error'])
        self.assertIs(sys.modules['py.path'], sys.modules['_pytest._py.path'])
        executed = []
        reports = []

        class AssertionItem(pytest.Item):
            def runtest(self):
                assert 6 * 7 == 42
                executed.append(self.nodeid)

        class OneAssertion:
            @pytest.hookimpl(tryfirst=True)
            def pytest_collection(self, session):
                session.items = [AssertionItem.from_parent(
                    session, name='admitted_dependency_assertion')]
                session.testscollected = 1
                return True

            def pytest_runtest_logreport(self, report):
                if report.when == 'call':
                    reports.append(report.outcome)

        config = self.root / 'pytest.ini'
        config.write_text('[pytest]\naddopts=\n')
        output = io.StringIO()
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
            status = pytest.main([
                '-q', '-c', str(config), '--disable-plugin-autoload',
                '-p', 'no:cacheprovider', '--noconftest', str(self.root),
            ], plugins=[OneAssertion()])
        self.assertEqual(status, 0, output.getvalue())
        self.assertEqual(len(executed), 1)
        self.assertEqual(reports, ['passed'])
        self.assertIn('1 passed', output.getvalue())
        owner.verify()

    def test_foreign_bytecode_cache_is_not_executed(self):
        owner = self.owner()
        prefix = sys.pycache_prefix
        sys.pycache_prefix = str(self.root / 'foreign-cache')
        try:
            source = owner.paths.bundle_root / 'pytest/__init__.py'
            cache = Path(importlib.util.cache_from_source(str(source)))
            cache.parent.mkdir(parents=True, mode=0o700)
            info = source.stat()
            poison = compile("raise AssertionError('foreign bytecode executed')\n",
                             str(source), 'exec')
            cache.write_bytes(importlib.util.MAGIC_NUMBER + struct.pack(
                '<III', 0, int(info.st_mtime) & 0xffffffff, info.st_size)
                + marshal.dumps(poison))
            self.assertEqual(owner.load().__version__, '9.0.3')
            owner.verify()
            self.assertTrue(cache.is_file())
        finally:
            sys.pycache_prefix = prefix

    def test_preexisting_test_namespace_is_rejected(self):
        owner = self.owner()
        self.inject('pytest.foreign', types.ModuleType('pytest.foreign'))
        with self.assertRaises(m.ScalingBootstrapError):
            owner.load()

    def test_extra_loaded_namespace_is_rejected(self):
        owner = self.owner()
        owner.load()
        self.inject('pytest.foreign', types.ModuleType('pytest.foreign'))
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_deleted_error_alias_is_rejected(self):
        owner = self.owner()
        owner.load()
        del sys.modules['py.error']
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_deleted_path_alias_is_rejected(self):
        owner = self.owner()
        owner.load()
        del sys.modules['py.path']
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_changed_alias_target_is_rejected(self):
        owner = self.owner()
        owner.load()
        self.inject('py.error', types.ModuleType('py.error'))
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_arbitrary_pygments_replacement_is_rejected(self):
        owner = self.owner()
        owner.load()
        original = sys.modules['pygments.lexers']
        replacement = type(original)('pygments.lexers')
        replacement.__dict__.update(original.__dict__)
        self.inject('pygments.lexers', replacement)
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_unadmitted_submodule_does_not_fall_back_to_path(self):
        owner = self.owner()
        owner.load()
        with self.assertRaises(ModuleNotFoundError):
            importlib.import_module('pytest.unadmitted_dependency_member')
        owner.verify()

    def test_missing_member_is_rejected(self):
        (self.source / 'pytest/__init__.py').unlink()
        self.assert_admission_rejected()

    def test_extra_member_is_rejected(self):
        (self.source / 'pytest/extra.py').write_text('raise AssertionError\n')
        self.assert_admission_rejected()

    def test_extra_empty_directory_is_rejected(self):
        (self.source / 'empty').mkdir(mode=0o700)
        self.assert_admission_rejected()

    def test_symlink_member_is_rejected(self):
        path = self.source / 'pytest/__init__.py'
        path.unlink()
        path.symlink_to('../_pytest/__init__.py')
        self.assert_admission_rejected()

    def test_hardlinked_member_is_rejected(self):
        os.link(self.source / 'pytest/__init__.py', self.root / 'second-link')
        self.assert_admission_rejected()

    def test_writable_by_other_users_is_rejected(self):
        (self.source / 'pytest/__init__.py').chmod(0o666)
        self.assert_admission_rejected()

    def test_record_hash_mismatch_is_rejected(self):
        path = self.source / 'pytest-9.0.3.dist-info/RECORD'
        rows = list(csv.reader(io.StringIO(path.read_text(), newline='')))
        row = next(row for row in rows if row[0] == 'pytest/__init__.py')
        row[1] = 'sha256=' + 'A' * 43
        rendered = io.StringIO(newline='')
        csv.writer(rendered, lineterminator='\n').writerows(rows)
        self.rewrite(path, rendered.getvalue().encode())
        self.assert_admission_rejected()

    def test_forged_source_with_refreshed_record_is_rejected(self):
        self.refresh_record('pytest/__init__.py', b'# forged pytest source\n')
        self.assert_admission_rejected()

    def test_forged_bundle_with_fresh_inventory_digest_is_rejected(self):
        self.refresh_record('pytest/__init__.py', b'# forged pytest source\n')
        bundle = self.root / 'forged-bundle'
        inventory = self.root / 'forged-inventory.json'
        m.bundle_contract.copy_private_bundle(self.source, bundle, inventory)
        paths = m.PythonDependencyPaths(self.source, bundle, inventory,
                                       hashlib.sha256(inventory.read_bytes()).hexdigest())
        with self.assertRaises(m.ScalingBootstrapError):
            m.PythonTestDependencies.admit(paths)

    def test_original_source_change_latches_failure(self):
        owner = self.owner()
        path = self.source / 'pytest/__init__.py'
        original = path.read_bytes()
        self.rewrite(path, b'# changed after admission\n')
        with self.assertRaises(m.ScalingBootstrapError):
            owner.validate()
        self.rewrite(path, original)
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_original_bundle_change_is_rejected(self):
        owner = self.owner()
        path = owner.paths.bundle_root / 'pytest/__init__.py'
        self.rewrite(path, path.read_bytes() + b'\n# changed\n')
        path.chmod(0o400)
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_original_inode_replacement_is_rejected(self):
        owner = self.owner()
        path = self.source / 'pytest/__init__.py'
        original = path.read_bytes()
        path.rename(self.root / 'original-init.py')
        path.write_bytes(original)
        path.chmod(0o600)
        with self.assertRaises(m.ScalingBootstrapError):
            owner.validate()

    def test_wrong_inventory_anchor_is_rejected(self):
        owner = self.owner()
        paths = replace(owner.paths, inventory_sha256='f' * 64)
        with self.assertRaises(m.ScalingBootstrapError):
            m.PythonTestDependencies.admit(paths)

    def test_same_inode_writable_descriptor_reuse_is_rejected(self):
        owner = self.owner()
        held = next(row for row in owner._held
                    if row['path'] == self.source / 'pytest/__init__.py')
        replacement = os.open(held['path'], os.O_WRONLY | os.O_CLOEXEC)
        os.dup2(replacement, held['descriptor'], inheritable=False)
        os.close(replacement)
        self.assert_descriptor_rejected_and_preserved(owner, held['descriptor'])

    def test_inheritable_descriptor_reuse_is_rejected(self):
        owner = self.owner()
        held = owner._held[0]
        replacement = os.open(held['path'], os.O_RDONLY | os.O_CLOEXEC)
        os.dup2(replacement, held['descriptor'], inheritable=True)
        os.close(replacement)
        self.assert_descriptor_rejected_and_preserved(owner, held['descriptor'])

    def test_descriptor_status_flag_change_is_rejected(self):
        owner = self.owner()
        descriptor = owner._held[0]['descriptor']
        fcntl.fcntl(descriptor, fcntl.F_SETFL,
                    fcntl.fcntl(descriptor, fcntl.F_GETFL) ^ os.O_NONBLOCK)
        self.assert_descriptor_rejected_and_preserved(owner, descriptor)

    def test_retained_directory_descriptor_reuse_is_rejected(self):
        owner = self.owner()
        descriptor = owner._directories[self.source][0]
        replacement = os.open(self.source, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        os.dup2(replacement, descriptor, inheritable=True)
        os.close(replacement)
        self.assert_descriptor_rejected_and_preserved(owner, descriptor)

    def test_held_parent_descriptor_reuse_is_rejected(self):
        owner = self.owner()
        held = owner._held[0]
        replacement = os.open(held['path'].parent,
                              os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        os.dup2(replacement, held['parent_fd'], inheritable=True)
        os.close(replacement)
        self.assert_descriptor_rejected_and_preserved(owner, held['parent_fd'])

    def test_closed_owner_cannot_reopen(self):
        owner = self.owner()
        owner.close()
        with self.assertRaises(m.ScalingBootstrapError):
            owner.load()
        with self.assertRaises(m.ScalingBootstrapError):
            owner.verify()

    def test_blake3_runtime_owner_rejects_test_dependency_bundle(self):
        owner = self.owner()
        self.assertEqual(set(m._package_files()), {
            'blake3/__init__.py', 'blake3/__init__.pyi', 'blake3/py.typed',
            'blake3/blake3' + m._profile(), 'blake3-1.0.9.dist-info/METADATA',
            'blake3-1.0.9.dist-info/WHEEL', 'blake3-1.0.9.dist-info/RECORD',
            'blake3-1.0.9.dist-info/licenses/LICENSE',
        })
        with self.assertRaises(m.ScalingBootstrapError):
            m.PythonDependencies.admit(owner.paths)
        owner.verify()
