#!/usr/bin/env python3
"""Focused storage admission tests; no deployment, SSH, or secret inputs."""
from pathlib import Path
import importlib.util
import json
import os
import tempfile
import subprocess
import sys
from unittest import mock
import unittest

PATH = Path(__file__).resolve().parents[1] / 'taira_disk_capacity.py'
SPEC = importlib.util.spec_from_file_location('taira_disk_capacity', PATH)
capacity = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(capacity)


def plan(*rows):
    return {'schema': capacity.PLAN_SCHEMA, 'allocations': list(rows)}


def row(path='/data', size=0, inodes=0):
    return {'path': path, 'label': 'future allocation', 'bytes': size, 'inodes': inodes}


def fs(device=1, size=100, inodes=100):
    return {'device': device, 'anchor': '/data', 'fragment_bytes': 4096,
            'available_bytes': size, 'available_inodes': inodes}


class CapacityTests(unittest.TestCase):
    def test_cohost_paths_aggregate_before_admission(self):
        result = capacity.evaluate(plan(row('/a', 60), row('/b', 60)), inspect=lambda _: fs())
        self.assertFalse(result['passed'])
        self.assertEqual(len(result['filesystems']), 1)
        self.assertEqual(result['filesystems'][0]['required_bytes'], 120)

    def test_separate_devices_are_not_falsely_aggregated(self):
        result = capacity.evaluate(plan(row('/a', 60), row('/b', 60)),
                                   inspect=lambda path: fs(device=1 if str(path) == '/a' else 2))
        self.assertTrue(result['passed'])
        self.assertEqual(len(result['filesystems']), 2)

    def test_byte_room_does_not_hide_inode_exhaustion(self):
        result = capacity.evaluate(plan(row(size=10, inodes=101)), inspect=lambda _: fs())
        self.assertFalse(result['passed'])
        self.assertIn('inodes', result['errors'][0])

    def test_available_space_uses_lowest_observation(self):
        observations = iter([fs(size=100), fs(size=50)])
        result = capacity.evaluate(plan(row('/a', 30), row('/b', 30)), inspect=lambda _: next(observations))
        self.assertFalse(result['passed'])
        self.assertEqual(result['filesystems'][0]['available_bytes'], 50)

    def test_equal_bound_passes_without_reservation(self):
        result = capacity.evaluate(plan(row(size=100, inodes=100)), inspect=lambda _: fs())
        self.assertTrue(result['passed'])
        self.assertFalse(result['reservation_created'])

    def test_invalid_fields_numbers_and_paths_fail(self):
        invalid = [row(size=True), row(inodes=-1), row(size=capacity.MAX_COUNT + 1),
                   row(path='relative'), row(path='/a/../b'), row(path='/a//b'),
                   {**row(), 'secret': 'not accepted'}]
        for item in invalid:
            with self.subTest(item=item), self.assertRaises(capacity.CapacityError):
                capacity.evaluate(plan(item))

    def test_aggregate_overflow_is_not_wrapped(self):
        with self.assertRaises(capacity.CapacityError):
            capacity.evaluate(plan(row(size=capacity.MAX_COUNT), row(size=1)), inspect=lambda _: fs())

    def test_future_directory_uses_actual_parent_filesystem(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            before = set(root.iterdir())
            observed = capacity.inspect_filesystem(root / 'future' / 'store')
            self.assertEqual(observed['device'], root.stat().st_dev)
            self.assertEqual(observed['anchor'], str(root))
            self.assertEqual(set(root.iterdir()), before)

    def test_symlink_component_and_file_anchor_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            (root / 'link').symlink_to(root, target_is_directory=True)
            (root / 'file').write_text('public')
            for path in (root / 'link' / 'new', root / 'file'):
                with self.subTest(path=path), self.assertRaises(capacity.CapacityError):
                    capacity.inspect_filesystem(path)

    def test_actual_statvfs_uses_unprivileged_available(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            expected = os.statvfs(root)
            result = capacity.inspect_filesystem(root)
            self.assertGreaterEqual(result['available_bytes'], 0)
            self.assertEqual(result['available_inodes'], expected.f_favail)
            self.assertEqual(result['fragment_bytes'], expected.f_frsize or expected.f_bsize)

    def test_allocation_bound_counts_sparse_payload_and_small_file_slack(self):
        self.assertEqual(capacity.allocation_bound(1000, 2, 3, 4096),
                         {'bytes': 1000 + 2 * 4095 + 3 * 4096, 'inodes': 5})
        with self.assertRaises(capacity.CapacityError):
            capacity.allocation_bound(1000, 0, 0, 4096)

    def test_full_fresh_cohost_copy_geometry_and_headroom(self):
        result = capacity.cohost_peak_plan(
            coordinator_path='/journal', upload_path='/srv', service_path='/srv',
            store_paths=['/state/a', '/state/b', '/state/c', '/state/d'],
            runtime_paths=['/runtime/a', '/runtime/b', '/runtime/c', '/runtime/d'],
            artifacts={'bytes': 100, 'inodes': 10}, stage={'bytes': 200, 'inodes': 20},
            per_store={'bytes': 300, 'inodes': 30},
            per_replica_runtime={'bytes': 400, 'inodes': 40}, headroom=[row('/journal', 500, 50)])
        self.assertEqual(sum(r['bytes'] for r in result['allocations']), 3 * 100 + 2 * 200 + 4 * 300 + 4 * 400 + 500)
        self.assertEqual(sum(r['inodes'] for r in result['allocations']), 3 * 10 + 2 * 20 + 4 * 30 + 4 * 40 + 50)
        # Space sufficient for all preseed copies still fails before any writes
        # if it cannot hold the later runtime guest hydration and writable disks.
        observed = capacity.evaluate(result, inspect=lambda _: fs(size=2400, inodes=1000))
        self.assertFalse(observed['passed'])

    def test_runtime_footprint_is_required_and_paths_cannot_alias(self):
        args = dict(coordinator_path='/journal', upload_path='/srv', service_path='/srv',
                    store_paths=['/state/a', '/state/b', '/state/c', '/state/d'],
                    artifacts={'bytes': 1, 'inodes': 1}, stage={'bytes': 1, 'inodes': 1},
                    per_store={'bytes': 1, 'inodes': 1}, headroom=[])
        with self.assertRaises(TypeError):
            capacity.cohost_peak_plan(**args)
        args['per_replica_runtime'] = {'bytes': 1, 'inodes': 1}
        for paths in (['/runtime/a'] * 4, ['/runtime/a'] * 3):
            with self.subTest(paths=paths), self.assertRaisesRegex(capacity.CapacityError, 'runtime paths'):
                capacity.cohost_peak_plan(**args, runtime_paths=paths)

    def test_runtime_allocations_follow_their_actual_filesystem(self):
        result = capacity.cohost_peak_plan(
            coordinator_path='/journal', upload_path='/srv', service_path='/srv',
            store_paths=['/state/a', '/state/b', '/state/c', '/state/d'],
            runtime_paths=['/runtime/a', '/runtime/b', '/runtime/c', '/runtime/d'],
            artifacts={'bytes': 1, 'inodes': 1}, stage={'bytes': 1, 'inodes': 1},
            per_store={'bytes': 1, 'inodes': 1},
            per_replica_runtime={'bytes': 20, 'inodes': 20}, headroom=[])
        observed = capacity.evaluate(result, inspect=lambda path:
                                     fs(device=2, size=100, inodes=79) if str(path).startswith('/runtime/')
                                     else fs(device=1, size=100, inodes=100))
        self.assertFalse(observed['passed'])
        self.assertEqual(len(observed['filesystems']), 2)
        self.assertIn('inodes', observed['errors'][0])

    def test_guest_free_does_not_override_full_sparse_backing_host(self):
        allocation = plan(row('/disk', 80))
        self.assertTrue(capacity.evaluate(allocation, inspect=lambda _: fs(size=100))['passed'])
        self.assertFalse(capacity.evaluate(allocation, inspect=lambda _: fs(size=70))['passed'])

    def test_fifo_rejection_is_bounded_without_writer(self):
        with tempfile.TemporaryDirectory() as tmp:
            fifo = Path(tmp) / 'plan.fifo'
            os.mkfifo(fifo)
            result = subprocess.run([sys.executable, str(PATH), '--plan', str(fifo)],
                                    capture_output=True, text=True, timeout=3)
            self.assertEqual(result.returncode, 2)
            self.assertIn('regular file', result.stdout)

    def test_oversized_plan_is_rejected_before_payload_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'large.json'
            with p.open('wb') as stream:
                stream.truncate(capacity.MAX_PLAN_BYTES + 1)
            with mock.patch.object(capacity.os, 'fdopen', side_effect=AssertionError('must not read')):
                with self.assertRaisesRegex(capacity.CapacityError, '1 MiB'):
                    capacity.read_plan(p)

    def test_reader_rejects_duplicate_fields_and_symlink(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            p = root / 'plan.json'
            p.write_text('{"schema":"a","schema":"b"}')
            with self.assertRaises(capacity.CapacityError):
                capacity.read_plan(p)
            p.write_text(json.dumps(plan(row(str(root)))))
            self.assertEqual(capacity.read_plan(p)['schema'], capacity.PLAN_SCHEMA)
            link = root / 'link.json'
            link.symlink_to(p)
            with self.assertRaises(OSError):
                capacity.read_plan(link)


if __name__ == '__main__':
    unittest.main()
