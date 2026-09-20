"""Actual bounded JSON and inherited descriptor tests without child processes."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

_ROOT = Path(__file__).resolve().parents[2]
_PATH = _ROOT / 'scripts/nexus/scaling_experiment_cli_inputs.py'
_SPEC = importlib.util.spec_from_file_location('scaling_experiment_cli_inputs', _PATH)
inputs = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = inputs
_SPEC.loader.exec_module(inputs)


def launch_value():
    """Complete scalar launch envelope; runtime decoding has separate admission."""
    return dict(schema=inputs.LAUNCH_SCHEMA,
        runtime_paths={'python_entrypoint': 'scripts/nexus/run_multilane_scaling_gate.py'},
        python_dependencies=dict(source_root='/private/source', bundle_root='/private/bundle',
            inventory='/private/inventory.json', inventory_sha256='a' * 64),
        plan=dict(path='/private/plan.json', sha256='b' * 64),
        budget=dict(path='/private/budget.json', sha256='c' * 64),
        evidence_root='/private/evidence', runtime_root='/private/runtime',
        worker_sources='/private/workers',
        identity=dict(machine_id='lab-1', storage_model='SSD', source_revision='d' * 40))


class LaunchJsonTests(unittest.TestCase):
    """Whole framing before decoding and rejection of alternate input shapes."""
    def test_readable_equivalent_input(self):
        value = launch_value()
        for raw in (json.dumps(value).encode(), json.dumps(value, indent=2).encode(),
                    json.dumps(dict(reversed(list(value.items())))).encode()):
            self.assertEqual(inputs.load_launch_value(raw), value)

    def test_exact_byte_ceiling(self):
        raw = json.dumps(launch_value()).encode()
        padded = raw + b' ' * (inputs.MAX_LAUNCH_BYTES - len(raw))
        self.assertEqual(inputs.load_launch_value(padded), launch_value())
        with self.assertRaises(inputs.LaunchInputError):
            inputs.load_launch_value(padded + b' ')

    def test_duplicate_escaped_top_level_key(self):
        raw = json.dumps(launch_value()).encode()
        duplicate = b'{"sch\\u0065ma":"' + inputs.LAUNCH_SCHEMA.encode() + b'",' + raw[1:]
        with self.assertRaises(inputs.LaunchInputError):
            inputs.load_launch_value(duplicate)

    def test_duplicate_nested_key(self):
        raw = json.dumps(launch_value()).encode().replace(
            b'"machine_id": "lab-1"', b'"machine_id":"first","machine_id":"second"')
        with self.assertRaises(inputs.LaunchInputError):
            inputs.load_launch_value(raw)

    def test_rejects_unknown_missing_or_retired_top_level_fields(self):
        for name in ('trial_command', 'qualified', 'schema', 'runtime_paths', 'plan', 'budget'):
            value = launch_value()
            if name in value:
                del value[name]
            else:
                value[name] = 'yes'
            with self.subTest(name=name), self.assertRaises(inputs.LaunchInputError):
                inputs.load_launch_value(json.dumps(value).encode())

    def test_rejects_old_schema_and_nonstring_leafs(self):
        cases = [None, True, False, 1, 1.5, [], {}, '']
        for item in cases:
            value = launch_value()
            value['identity']['machine_id'] = item
            # An empty object is structurally allowed and later exact decoding
            # rejects it; the framing boundary does not claim typed admission.
            if item == {}:
                continue
            with self.subTest(item=item), self.assertRaises(inputs.LaunchInputError):
                inputs.load_launch_value(json.dumps(value).encode())
        for schema in ('v0', 'v2', inputs.LAUNCH_SCHEMA + '.legacy'):
            value = launch_value(); value['schema'] = schema
            with self.subTest(schema=schema), self.assertRaises(inputs.LaunchInputError):
                inputs.load_launch_value(json.dumps(value).encode())

    def test_rejects_truncation_trailing_data_and_nonascii(self):
        raw = json.dumps(launch_value()).encode()
        for item in (raw[:-1], raw + b'{}', b'\xff' + raw, b'[]', b'null', b'{}', b'"x"'):
            with self.subTest(raw=item[:10]), self.assertRaises(inputs.LaunchInputError):
                inputs.load_launch_value(item)

    def test_depth_and_scalar_bounds(self):
        for value in ({'a': {'b': {'c': 'x'}}}, 'x' * 4097, '\U0001f600' * 4097):
            launch = launch_value(); launch['identity']['machine_id'] = value
            with self.subTest(kind=type(value)), self.assertRaises(inputs.LaunchInputError):
                inputs.load_launch_value(json.dumps(launch).encode())


class DescriptorTests(unittest.TestCase):
    """Retain exact inherited handles; reject wrong modes, mutation and partial pipes."""
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.fds = []

    def tearDown(self):
        for fd in self.fds:
            try:
                os.close(fd)
            except OSError:
                pass
        self.directory.cleanup()

    def regular(self, data, mode=0o600, access=os.O_RDONLY):
        path = self.root / f'input-{len(self.fds)}'
        path.write_bytes(data); path.chmod(mode)
        fd = os.open(path, access); self.fds.append(fd)
        return path, fd

    def pipe(self, data, *, blocking=False, eof=True):
        reader, writer = os.pipe()
        self.fds.extend((reader, writer))
        os.set_blocking(reader, blocking)
        if data:
            os.write(writer, data)
        if eof:
            os.close(writer); self.fds.remove(writer)
        return reader

    def test_launch_exact_original_read_and_close(self):
        raw = json.dumps(launch_value()).encode()
        path, fd = self.regular(raw, 0o400)
        os.lseek(fd, len(raw), os.SEEK_SET)
        self.assertEqual(inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest()), raw)
        with self.assertRaises(OSError):
            os.fstat(fd)
        self.assertEqual(path.read_bytes(), raw)

    def test_launch_hash_rejection_closes_admitted_descriptor(self):
        _, fd = self.regular(b'{}')
        with self.assertRaises(inputs.LaunchInputError):
            inputs.read_launch_descriptor(fd, '0' * 64)
        with self.assertRaises(OSError):
            os.fstat(fd)

    def test_launch_rejects_wrong_ownership_shape(self):
        for mode, access, size in ((0o644, os.O_RDONLY, 2), (0o600, os.O_RDWR, 2),
                                  (0o600, os.O_RDONLY, 0),
                                  (0o600, os.O_RDONLY, inputs.MAX_LAUNCH_BYTES + 1)):
            _, fd = self.regular(b'x' * size, mode, access)
            with self.subTest(mode=mode, access=access, size=size), self.assertRaises(inputs.LaunchInputError):
                inputs.read_launch_descriptor(fd, 'a' * 64)

    def test_launch_rejects_hardlink(self):
        path, fd = self.regular(b'{}')
        os.link(path, self.root / 'second-name')
        with self.assertRaises(inputs.LaunchInputError):
            inputs.read_launch_descriptor(fd, hashlib.sha256(b'{}').hexdigest())

    def test_launch_rejects_rewrite_during_final_read(self):
        raw = b'{}'; path, fd = self.regular(raw)
        original = os.pread
        def read_then_rewrite(*args):
            data = original(*args)
            path.write_bytes(b'[]')
            return data
        with patch.object(inputs.os, 'pread', read_then_rewrite), self.assertRaises(inputs.LaunchInputError):
            inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest())

    def test_launch_does_not_close_reused_foreign_descriptor(self):
        raw = b'{}'; _, fd = self.regular(raw)
        foreign_path, foreign = self.regular(b'foreign')
        original = os.pread
        def read_then_replace(*args):
            data = original(*args)
            os.dup2(foreign, fd)
            return data
        with patch.object(inputs.os, 'pread', read_then_replace), self.assertRaises(inputs.LaunchInputError):
            inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest())
        self.assertEqual(os.pread(fd, 7, 0), b'foreign')
        self.assertEqual(os.fstat(fd).st_ino, foreign_path.stat().st_ino)

    def test_seed_ready_pipe_consumed_and_closed(self):
        fd = self.pipe(b'a' * 64)
        self.assertEqual(inputs.read_seed_descriptor(fd), 'a' * 64)
        with self.assertRaises(OSError):
            os.fstat(fd)

    def test_seed_rejects_partial_extra_uppercase_or_nonhex(self):
        for raw in (b'', b'a' * 63, b'a' * 65, b'a' * 64 + b'\n', b'A' * 64, b'z' * 64):
            fd = self.pipe(raw)
            with self.subTest(raw=raw[:4]), self.assertRaises(inputs.LaunchInputError):
                inputs.read_seed_descriptor(fd)

    def test_seed_requires_ready_eof_without_wait(self):
        for raw in (b'', b'a' * 64):
            fd = self.pipe(raw, eof=False)
            with self.subTest(length=len(raw)), self.assertRaises(inputs.LaunchInputError):
                inputs.read_seed_descriptor(fd)

    def test_seed_rejects_blocking_or_regular_inputs(self):
        _, regular = self.regular(b'a' * 64)
        blocking = self.pipe(b'a' * 64, blocking=True)
        for fd in (regular, blocking):
            with self.subTest(fd=fd), self.assertRaises(inputs.LaunchInputError):
                inputs.read_seed_descriptor(fd)

    def test_descriptor_scalar_types_and_std_handles_rejected(self):
        for fd in (True, False, -1, 0, 1, 2, 1 << 20, '3', None):
            with self.subTest(fd=fd), self.assertRaises(inputs.LaunchInputError):
                inputs.read_seed_descriptor(fd)


if __name__ == '__main__':
    unittest.main()


class BoundaryReviewTests(unittest.TestCase):
    """Actual descriptor regressions; no children, native launches or signals."""
    setUp, tearDown = DescriptorTests.setUp, DescriptorTests.tearDown
    regular, pipe = DescriptorTests.regular, DescriptorTests.pipe

    def test_launch_reuse_during_hash_rejected_without_closing_foreign(self):
        raw = b'{}'; _, fd = self.regular(raw)
        _, foreign = self.regular(b'foreign-secret')
        expected = hashlib.sha256(raw).hexdigest(); original = hashlib.sha256
        def hash_then_replace(*args, **kwargs):
            digest = original(*args, **kwargs)
            os.dup2(foreign, fd)
            return digest
        with patch.object(inputs.hashlib, 'sha256', hash_then_replace):
            with self.assertRaisesRegex(inputs.LaunchInputError, '^fixed_scaling_launch_invalid$'):
                inputs.read_launch_descriptor(fd, expected)
        self.assertEqual(os.pread(fd, 14, 0), b'foreign-secret')

    def test_launch_reuse_after_final_stat_snapshot_rejected_without_closing_foreign(self):
        raw = b'{}'; _, fd = self.regular(raw)
        _, foreign = self.regular(b'foreign-secret')
        original = os.fstat; calls = []
        def snapshot_then_replace(value):
            snapshot = original(value)
            if value == fd:
                calls.append(value)
                if len(calls) == 2:
                    os.dup2(foreign, fd, inheritable=False)
            return snapshot
        with patch.object(inputs.os, 'fstat', snapshot_then_replace):
            with self.assertRaisesRegex(inputs.LaunchInputError, '^fixed_scaling_launch_invalid$'):
                inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest())
        self.assertEqual(os.pread(fd, 14, 0), b'foreign-secret')

    def test_same_inode_writable_reuse_rejected_and_left_open(self):
        raw = b'{}'; path, fd = self.regular(raw)
        sibling = os.open(path, os.O_RDWR); self.fds.append(sibling)
        original = os.pread
        def read_then_replace(*args):
            data = original(*args); os.dup2(sibling, fd, inheritable=False); return data
        with patch.object(inputs.os, 'pread', read_then_replace):
            with self.assertRaisesRegex(inputs.LaunchInputError, '^fixed_scaling_launch_invalid$'):
                inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest())
        self.assertEqual(os.fstat(fd).st_ino, path.stat().st_ino)
        import fcntl
        self.assertEqual(fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE, os.O_RDWR)

    def test_seed_final_nonblocking_mode_change_rejected_and_left_open(self):
        fd = self.pipe(b'a' * 64); original = os.read
        def read_then_change(*args):
            data = original(*args)
            if args == (fd, 1):
                os.set_blocking(fd, True)
            return data
        with patch.object(inputs.os, 'read', read_then_change):
            with self.assertRaisesRegex(inputs.LaunchInputError, '^fixed_scaling_launch_invalid$'):
                inputs.read_seed_descriptor(fd)
        self.assertTrue(os.get_blocking(fd))

    def test_launch_bounded_reads_and_short_eof(self):
        raw = b'x' * inputs.MAX_LAUNCH_BYTES; _, fd = self.regular(raw)
        original = os.pread; calls = []
        def record(fd, count, offset):
            calls.append((count, offset)); return original(fd, count, offset)
        with patch.object(inputs.os, 'pread', record):
            self.assertEqual(inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest()), raw)
        self.assertEqual(calls, [(4096, i) for i in range(0, len(raw), 4096)])
        path, fd = self.regular(raw); calls = []
        def truncate_after_read(fd, count, offset):
            calls.append((count, offset)); data = original(fd, count, offset)
            if offset == 0:
                path.write_bytes(b'')
            return data
        with patch.object(inputs.os, 'pread', truncate_after_read):
            with self.assertRaises(inputs.LaunchInputError):
                inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest())
        self.assertEqual(calls, [(4096, 0), (4096, 4096)])

    def test_seed_reads_bounded_and_exactly_once(self):
        fd = self.pipe(b'a' * 64); original = os.read; calls = []
        def record(fd, count):
            calls.append(count); return original(fd, count)
        with patch.object(inputs.os, 'read', record):
            self.assertEqual(inputs.read_seed_descriptor(fd), 'a' * 64)
            with self.assertRaises(inputs.LaunchInputError):
                inputs.read_seed_descriptor(fd)
        self.assertEqual(calls, [65, 1])

    def test_sibling_descriptor_kept_open_after_consumption(self):
        raw = b'{}'; _, fd = self.regular(raw)
        sibling = os.dup(fd); self.fds.append(sibling)
        self.assertEqual(inputs.read_launch_descriptor(fd, hashlib.sha256(raw).hexdigest()), raw)
        self.assertEqual(os.pread(sibling, 2, 0), raw)
        with self.assertRaises(inputs.LaunchInputError):
            inputs.read_seed_descriptor(sibling)
        self.assertEqual(os.pread(sibling, 2, 0), raw)

    def test_closed_diagnostics_do_not_leak_input_or_pipe_secret(self):
        from contextlib import redirect_stdout, redirect_stderr
        from io import StringIO
        out, err = StringIO(), StringIO()
        secret = 'secret-private-runtime-key'
        fd = self.pipe(secret.encode())
        with redirect_stdout(out), redirect_stderr(err):
            for function, args in ((inputs.read_seed_descriptor, (fd,)),
                (inputs.load_launch_value, (json.dumps({'secret': secret}).encode(),))):
                try:
                    function(*args)
                except inputs.LaunchInputError as error:
                    self.assertEqual(str(error), 'fixed_scaling_launch_invalid')
                    self.assertTrue(error.__suppress_context__)
                else:
                    self.fail('malformed input was accepted')
        self.assertEqual(out.getvalue(), ''); self.assertEqual(err.getvalue(), '')


class TypedDecodeReviewTests(unittest.TestCase):
    """Real RuntimePaths + explicit dependency-dataclass shape-only seam.

    These assert input decoding only, never dependency or runtime admission.
    """
    def setUp(self):
        from dataclasses import dataclass
        from types import ModuleType
        import scaling_runtime_admission
        self.runtime = scaling_runtime_admission.ReleaseRuntimePaths
        @dataclass(frozen=True, slots=True)
        class PythonDependencyPaths:
            source_root: Path
            bundle_root: Path
            inventory: Path
            inventory_sha256: str
        self.dependency = PythonDependencyPaths
        seam = ModuleType('scaling_cli_bootstrap'); seam.PythonDependencyPaths = PythonDependencyPaths
        self.override = patch.dict(sys.modules, {'scaling_cli_bootstrap': seam})
        self.override.start(); self.addCleanup(self.override.stop)

    def value(self):
        from dataclasses import fields
        value = launch_value()
        value['runtime_paths'] = {field.name: ('a' * 64 if field.name.endswith('sha256') else
            'scripts/nexus/run_multilane_scaling_gate.py' if field.name == 'python_entrypoint' else
            '/private/' + field.name) for field in fields(self.runtime)}
        return value

    def test_actual_runtime_dataclass_shape(self):
        for revision in ('d' * 40, 'd' * 64):
            value = self.value(); value['identity']['source_revision'] = revision
            decoded = inputs.decode_launch_input(json.dumps(value).encode())
            self.assertIs(type(decoded.runtime_paths), self.runtime)
            self.assertIs(type(decoded.python_dependencies), self.dependency)
            self.assertEqual(decoded.plan_path, Path('/private/plan.json'))
            self.assertEqual(decoded.source_revision, revision)

    def test_typed_nonstring_leafs_empty_objects_and_unknown_nested_fields(self):
        for group in ('runtime_paths', 'python_dependencies', 'plan', 'budget', 'identity'):
            for mutation in ('missing', 'extra', 'object'):
                value = self.value(); field = next(iter(value[group]))
                if mutation == 'missing': del value[group][field]
                elif mutation == 'extra': value[group]['retired'] = 'value'
                else: value[group][field] = {}
                with self.subTest(group=group, mutation=mutation):
                    with self.assertRaisesRegex(inputs.LaunchInputError, '^fixed_scaling_launch_invalid$'):
                        inputs.decode_launch_input(json.dumps(value).encode())

    def test_noncanonical_paths_and_labels(self):
        bad_paths = ('relative', '/private/../input', '/private//input', '/private/./input',
            '/private/input/', '/private/\x00input', '/' + '/'.join(['x'] * 64), '/private/\ud800')
        for path in bad_paths:
            value = self.value(); value['plan']['path'] = path
            with self.subTest(path=ascii(path)), self.assertRaises(inputs.LaunchInputError):
                inputs.decode_launch_input(json.dumps(value).encode())
        for label in ('line\nbreak', '\u0080', 'x' * 513):
            value = self.value(); value['identity']['machine_id'] = label
            with self.subTest(label=ascii(label)), self.assertRaises(inputs.LaunchInputError):
                inputs.decode_launch_input(json.dumps(value).encode())

    def test_digest_revision_and_entrypoint_shape(self):
        for group, key, values in (
            ('runtime_paths', 'python_entrypoint', ('/scripts/nexus/run_multilane_scaling_gate.py', 'alternate.py')),
            ('plan', 'sha256', ('A' * 64, 'a' * 63, 'g' * 64)),
            ('identity', 'source_revision', ('A' * 40, 'd' * 39, 'd' * 41, 'd' * 65))):
            for item in values:
                value = self.value(); value[group][key] = item
                with self.subTest(group=group, key=key, item=item), self.assertRaises(inputs.LaunchInputError):
                    inputs.decode_launch_input(json.dumps(value).encode())

    def test_raw_types_rejected_before_user_callbacks(self):
        class ForeignBytes(bytes): pass
        for raw in (None, True, '{}', bytearray(b'{}'), memoryview(b'{}'), ForeignBytes(b'{}')):
            with self.subTest(kind=type(raw)), self.assertRaises(inputs.LaunchInputError):
                inputs.load_launch_value(raw)

    def test_deep_and_escaped_duplicate_json_rejected_before_typed_decode(self):
        raw = json.dumps(self.value()).encode()
        samples = [raw.replace(b'"machine_id": "lab-1"',
                b'"mach\\u0069ne_id":"secret","machine_id":"lab-1"'),
            raw.replace(b'"machine_id": "lab-1"', b'"machine_id":' + b'{"x":' * 5000 + b'"secret"' + b'}' * 5000),
            raw.replace(b'"machine_id": "lab-1"', b'"machine_id":NaN'),
            raw.replace(b'"machine_id": "lab-1"', b'"machine_id":1e999')]
        for raw in samples:
            with self.assertRaisesRegex(inputs.LaunchInputError, '^fixed_scaling_launch_invalid$'):
                inputs.decode_launch_input(raw)
