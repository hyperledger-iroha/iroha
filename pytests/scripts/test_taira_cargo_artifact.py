"""Bounded Cargo alias-capture regressions; disposable files, no Cargo or network."""
import contextlib
import hashlib
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import taira_cargo_artifact as cargo
import release_artifact_contract as contract


class CargoArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.profile = self.root / 'debug'; self.profile.mkdir(mode=0o700)
        self.deps = self.profile / 'deps'; self.deps.mkdir(mode=0o700)
        self.binary = self.profile / 'kagami'
        self.binary.write_bytes(b'public executable bytes'); self.binary.chmod(0o700)
        self.alias = self.deps / 'kagami-0123456789abcdef'

    def pair(self):
        os.link(self.binary, self.alias)
        return cargo.cargo_hash_path(self.binary, max_size=1024)

    def test_single_link_contract_remains_generic_and_pair_is_closed(self):
        one = cargo.cargo_hash_path(self.binary, max_size=1024)
        self.assertEqual(one, contract.stable_hash_path(self.binary))
        with cargo.cargo_open_relative(self.root, 'debug/kagami', expected=one) as source:
            self.assertEqual(os.read(source, 1024), self.binary.read_bytes())
        two = self.pair()
        with self.assertRaisesRegex(contract.ReleaseArtifactError, 'exactly one hard link'):
            contract.stable_hash_path(self.binary)
        self.assertEqual(two.link_count, 2)
        self.assertEqual(two.alias, self.alias.name)
        with cargo.cargo_open_relative(self.root, 'debug/kagami', expected=two) as source:
            output = self.root / 'captured'
            with output.open('xb') as dest: dest.write(os.read(source, 1024))
        self.assertEqual(output.stat().st_nlink, 1)
        self.assertNotEqual(output.stat().st_ino, self.binary.stat().st_ino)
        self.binary.write_bytes(b'new owned Cargo output')
        self.assertEqual(output.read_bytes(), b'public executable bytes')
        self.assertEqual(self.alias.stat().st_nlink, 2)

    def test_noncanonical_or_open_alias_topology_is_rejected(self):
        for variant in ('unknown', 'third', 'deps_source', 'short_hash', 'shared', 'symlink'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as directory:
                profile = Path(directory).resolve() / 'release'; profile.mkdir(mode=0o700)
                deps = profile / 'deps'; deps.mkdir(mode=0o700)
                binary = profile / 'kagami'; binary.write_bytes(b'candidate'); binary.chmod(0o700)
                alias = deps / ('kagami-a' if variant == 'short_hash' else 'kagami-0123456789abcdef')
                os.link(binary, profile / 'unexpected' if variant == 'unknown' else alias)
                if variant == 'third': os.link(binary, profile / 'third')
                if variant == 'shared': binary.chmod(0o770)
                if variant == 'symlink':
                    alias.unlink(); alias.symlink_to(binary); os.link(binary, profile / 'unexpected')
                reason = {'unknown': 'exact hashed deps alias', 'third': 'exactly two aliases',
                          'deps_source': 'canonical published', 'short_hash': 'exact hashed deps alias',
                          'shared': 'bounded owner-held', 'symlink': 'exact hashed deps alias'}[variant]
                with self.assertRaisesRegex(contract.ReleaseArtifactError, reason):
                    cargo.cargo_hash_path(alias if variant == 'deps_source' else binary, max_size=1024)

    def test_hyphenated_shipping_binary_requires_normalized_crate_alias(self):
        binary = self.profile / 'sorafs-node'
        self.binary.rename(binary)
        alias = self.deps / 'sorafs_node-0123456789abcdef'
        os.link(binary, alias)
        expected = cargo.cargo_hash_path(binary, max_size=1024)
        self.assertEqual(expected.alias, alias.name)
        with cargo.cargo_open_relative(self.root, 'debug/sorafs-node', expected=expected) as source:
            self.assertEqual(os.read(source, 1024), b'public executable bytes')
        alias.rename(self.deps / 'sorafs-node-0123456789abcdef')
        with self.assertRaisesRegex(contract.ReleaseArtifactError, 'exact hashed deps alias'):
            cargo.cargo_hash_path(binary, max_size=1024)

    def test_alias_topology_change_between_hash_and_copy_is_rejected(self):
        expected = self.pair()
        self.alias.rename(self.deps / 'kagami-fedcba9876543210')
        with self.assertRaisesRegex(contract.ReleaseArtifactError, 'identity or alias topology'):
            with cargo.cargo_open_relative(self.root, 'debug/kagami', expected=expected):
                self.fail('changed topology yielded an executable descriptor')

    def test_replacement_link_count_and_ancestor_changes_during_copy_are_rejected(self):
        for mutation in ('published', 'alias', 'third', 'deps', 'profile'):
            with self.subTest(mutation=mutation):
                fixture = CargoArtifactTests(); fixture.setUp()
                try:
                    expected = fixture.pair()
                    with self.assertRaises((contract.ReleaseArtifactError, OSError)):
                        with cargo.cargo_open_relative(fixture.root, 'debug/kagami', expected=expected) as source:
                            self.assertEqual(os.read(source, 1024), b'public executable bytes')
                            if mutation == 'third': os.link(fixture.binary, fixture.root / 'third')
                            elif mutation in ('published', 'alias'):
                                destination = fixture.binary if mutation == 'published' else fixture.alias
                                changed = fixture.root / 'replacement'; changed.write_bytes(b'public executable bytes')
                                changed.chmod(0o700); os.replace(changed, destination)
                            else:
                                directory = fixture.deps if mutation == 'deps' else fixture.profile
                                directory.rename(directory.with_name(directory.name + '-retained'))
                                directory.mkdir(mode=0o700)
                finally:
                    fixture.doCleanups()

    def test_same_size_rewrite_fails_digest_even_when_metadata_comparison_collides(self):
        expected = self.pair()
        identity = cargo._identity(self.binary.stat())
        with patch.object(cargo, '_identity', return_value=identity):
            with self.assertRaisesRegex(contract.ReleaseArtifactError, 'content changed during copy'):
                with cargo.cargo_open_relative(self.root, 'debug/kagami', expected=expected):
                    self.binary.write_bytes(b'x' * expected.size)

    def test_release_profile_pair_and_size_bound_use_same_owner(self):
        expected = self.pair()
        self.profile.rename(self.root / 'release')
        binary = self.root / 'release/kagami'
        with self.assertRaisesRegex(contract.ReleaseArtifactError, 'bounded owner-held'):
            cargo.cargo_hash_path(binary, max_size=expected.size - 1)
        actual = cargo.cargo_hash_path(binary, max_size=1024)
        self.assertEqual(actual.sha256, hashlib.sha256(b'public executable bytes').hexdigest())
        with cargo.cargo_open_relative(self.root, 'release/kagami', expected=actual) as source:
            self.assertEqual(os.read(source, 1024), b'public executable bytes')


if __name__ == '__main__':
    unittest.main()
