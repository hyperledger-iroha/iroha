"""Small clone/custody/capacity controls only; no real proving originals or processes."""
from pathlib import Path
from paths import QUAL
import hashlib
import json
import os
import stat
import tempfile
import unittest
from unittest.mock import patch
import sealed_data as s
import run as r


class SealedDataTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='controller8-data-', dir=QUAL)
        self.assertTrue(Path(self.temp.name).is_relative_to(QUAL))
        self.assertEqual(Path(self.temp.name).stat().st_mode & 0o777, 0o700)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.catalog = self.root / 'catalog'; self.catalog.mkdir(mode=0o700)
        self.raw = self.catalog / 'originals'; self.raw.mkdir(mode=0o700)
        self.bytes = b'Synthetic clone DATA only. No key, authority or receipt.'
        self.digest = hashlib.sha256(self.bytes).hexdigest()
        self.source = self.raw / self.digest; self.source.write_bytes(self.bytes)
        self.source.chmod(0o600)
        self.pins = dict.fromkeys(s.data.SOURCE_PINS, 'a' * 64)
        self.value = {'schema': s.MEMBERSHIP_SCHEMA, 'scope': 'Synthetic DATA',
            'wallet_originals': [{'sha256': self.digest, 'bytes': len(self.bytes)}],
            'wallet_unique_bytes': len(self.bytes)}
        self.membership = r.ref(r.save(self.catalog / 'qualified-source-membership.json', self.value))
        self.out = self.root / 'sealed'

    def prepare(self):
        return s.prepare(self.catalog, self.membership, self.pins, self.out)

    def test_actual_clone_is_independent_sealed_and_preserves_multilink_raw_source(self):
        # This deliberately exercises the real APFS syscall, not mocked copy success.
        os.link(self.source, self.root / 'retained-raw-link')
        before = s.identity(self.source.stat())
        selection, receipt = self.prepare()
        copied = self.out / 'originals' / self.digest
        self.assertEqual(copied.read_bytes(), self.bytes)
        self.assertNotEqual(copied.stat().st_ino, self.source.stat().st_ino)
        self.assertEqual(copied.stat().st_nlink, 1)
        self.assertEqual(stat.S_IMODE(copied.stat().st_mode), 0o400)
        self.assertEqual(s.identity(self.source.stat()), before)
        self.assertEqual(selection, {'schema': s.SELECTION_SCHEMA, 'source_pins': self.pins,
            'sealed_originals': {'schema': s.LOCATOR_SCHEMA, 'path': str(self.out / 'originals')}})
        self.assertEqual(receipt['membership'], self.membership)
        self.assertEqual(len(receipt['originals']), 1)
        self.assertFalse((self.out / 'selection.json').exists(), 'caller publishes only after successful return')

    def test_actual_tiny_probe_retains_independent_data_without_authority(self):
        got = s.probe(self.root / 'probe')
        self.assertNotEqual(got['source_identity'][1], got['clone_identity'][1])
        self.assertEqual(got['clone_identity'][4], 1)
        self.assertEqual(stat.S_IMODE(got['clone_identity'][2]), 0o400)

    def test_unsupported_clone_refuses_before_output_and_never_full_copies(self):
        with patch.object(s, 'clone_function', side_effect=ValueError('unavailable')):
            with self.assertRaisesRegex(ValueError, 'unavailable'): self.prepare()
        self.assertFalse(self.out.exists())
        with patch.object(s, 'clone_function', return_value=lambda *args: (_ for _ in ()).throw(OSError('EXDEV'))):
            with self.assertRaisesRegex(OSError, 'EXDEV'): self.prepare()
        self.assertEqual(list((self.out / 'originals').iterdir()), [])
        self.assertEqual(self.source.read_bytes(), self.bytes)

    def test_hardlink_substitution_refuses_before_hardening_raw(self):
        before = s.identity(self.source.stat())
        def link(fd, directory, name):
            os.link(self.source, name, dst_dir_fd=directory)
        with patch.object(s, 'clone_function', return_value=link):
            with self.assertRaisesRegex(ValueError, 'independent inode'): self.prepare()
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), stat.S_IMODE(before[2]))
        self.assertEqual(self.source.read_bytes(), self.bytes)

    def test_symlink_and_foreign_content_cannot_create_a_selection(self):
        real = self.root / 'real'; self.source.rename(real); self.source.symlink_to(real)
        with self.assertRaisesRegex(ValueError, 'type/owner/extent'): self.prepare()
        self.assertFalse((self.out / 'selection.json').exists())
        self.source.unlink(); self.source.write_bytes(b'!' * len(self.bytes)); self.source.chmod(0o600)
        self.out = self.root / 'changed-sealed'
        with self.assertRaisesRegex(ValueError, 'content hash'): self.prepare()
        self.assertFalse((self.out / 'selection.json').exists())

    def test_raw_change_during_clone_is_not_repaired_or_retried(self):
        real = s.clone_function(); calls = []
        def mutate(fd, directory, name):
            calls.append(name); real(fd, directory, name); os.chmod(self.source, 0o644)
        with patch.object(s, 'clone_function', return_value=mutate):
            with self.assertRaisesRegex(ValueError, 'raw original changed'): self.prepare()
        self.assertEqual(calls, [self.digest])
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), 0o644)
        self.assertTrue((self.out / 'originals' / self.digest).exists(), 'retain partial clone evidence')

    def test_missing_changed_membership_pins_and_existing_output_refuse(self):
        for pins in [{}, self.pins | {'old_grant': 'b' * 64}, self.pins | {s.data.SOURCE_PINS[0]: '0' * 64}]:
            with self.assertRaises(ValueError): s.prepare(self.catalog, self.membership, pins, self.out)
        self.assertFalse(self.out.exists())
        self.out.mkdir()
        with self.assertRaisesRegex(ValueError, 'fresh independent'): self.prepare()
        self.out.rmdir()
        with self.assertRaisesRegex(ValueError, 'selected original changed'):
            s.prepare(self.catalog, self.membership | {'sha256': 'b' * 64}, self.pins, self.out)
        self.source.unlink()
        with self.assertRaises(FileNotFoundError): self.prepare()
        self.assertFalse((self.out / 'selection.json').exists())

    def test_membership_shape_duplicates_bounds_and_totals_refuse(self):
        variants = [self.value | {'schema': 'old-admission'}, self.value | {'wallet_unique_bytes': True},
            self.value | {'wallet_originals': []},
            self.value | {'wallet_originals': self.value['wallet_originals'] * 2},
            self.value | {'wallet_unique_bytes': len(self.bytes) + 1},
            self.value | {'wallet_originals': [{'sha256': self.digest, 'bytes': s.FILE_BYTES + 1}]}]
        for value in variants:
            with self.subTest(value=value), self.assertRaises(ValueError): s.rows(value)

    def test_capacity_does_not_subtract_unproved_reuse_and_refuses_low_space(self):
        class Disk:
            free = s.RESERVE_BYTES + 1234
        with patch.object(s.shutil, 'disk_usage', return_value=Disk):
            got = s.capacity(self.root)
        self.assertEqual(got['maximum_new_raw_bytes'], 1234)
        self.assertFalse(got['full_copy_fallback'])
        Disk.free = s.RESERVE_BYTES - 1
        with patch.object(s.shutil, 'disk_usage', return_value=Disk):
            with self.assertRaisesRegex(ValueError, 'headroom'): self.prepare()
        self.assertFalse(self.out.exists())

    def test_low_space_mid_transport_preserves_partial_data_and_no_selection(self):
        actual = s.capacity(self.root); calls = []
        def capacity(_):
            calls.append(None)
            if len(calls) == 3: raise ValueError('headroom lost')
            return actual
        with patch.object(s, 'capacity', side_effect=capacity):
            with self.assertRaisesRegex(ValueError, 'headroom lost'): self.prepare()
        self.assertTrue((self.out / 'originals' / self.digest).exists())
        self.assertFalse((self.out / 'selection.json').exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
