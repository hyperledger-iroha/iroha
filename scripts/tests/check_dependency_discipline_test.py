"""Exercise dependency discipline through Bash and its real local-edge validator.

Requires Python 3.11+, Git, Bash and Cargo. Tests use synthetic Git tree bases in
temporary workspaces and run only locked offline Cargo metadata; they never make
commits, build or fetch packages, modify the real checkout, or inherit external
dependency approval from the host.
Run with python3 -B -m unittest discover -s scripts/tests
-p check_dependency_discipline_test.py -v.
"""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class DependencyDisciplineTests(unittest.TestCase):
    """Use actual manifest/lock changes and the actual shell approval boundary."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='dependency-discipline-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith('GIT_') and key not in ('DEPENDENCY_DISCIPLINE_ALLOW', 'AGENTS_BASE_REF', 'LOCAL_EDGES')}
        self.environment['PYTHONDONTWRITEBYTECODE'] = '1'
        for name in ('ci/check_dependency_discipline.sh', 'ci/lib/base_ref.sh', 'scripts/check_agents_local_dependency_edges.py'):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        (self.root / 'Cargo.toml').write_text('[workspace]\nmembers = ["xtask", "crates/fastpq_isi", "crates/other"]\nresolver = "2"\n')
        for name, relative in [('xtask', 'xtask'), ('fastpq_isi', 'crates/fastpq_isi'), ('other', 'crates/other')]:
            package = self.root / relative
            (package / 'src').mkdir(parents=True)
            (package / 'src/lib.rs').write_text('// Metadata-only integration fixture.\n')
            (package / 'Cargo.toml').write_text(f'[package]\nname = "{name}"\nversion = "1.0.0"\nedition = "2021"\n')
        self.base_lock = 'version = 4\n' + ''.join(f'\n[[package]]\nname = "{name}"\nversion = "1.0.0"\n' for name in ('fastpq_isi', 'other', 'xtask'))
        (self.root / 'Cargo.lock').write_text(self.base_lock)
        self.git('init', '-q')
        self.git('add', '.')
        # git show <tree>:<path> reads the exact base without creating a commit,
        # requiring signing credentials or changing signing/hook configuration.
        self.environment['AGENTS_BASE_REF'] = self.git('write-tree').strip()

    def git(self, *arguments):
        """Run Git only inside this disposable fixture without global config writes."""
        return subprocess.check_output(['git', '-C', str(self.root), *arguments], env=self.environment, text=True)

    def append(self, relative, text):
        """Append a candidate manifest declaration in the disposable fixture."""
        with (self.root / relative).open('a') as handle:
            handle.write(text)

    def add_local_edge(self):
        """Represent the reviewed xtask-to-existing-local-package case coherently."""
        self.append('xtask/Cargo.toml', '\n[dependencies]\nfastpq_isi = { path = "../crates/fastpq_isi" }\n')
        (self.root / 'Cargo.lock').write_text(self.base_lock + 'dependencies = ["fastpq_isi"]\n')

    def check(self, *, allow=None):
        """Invoke the real shell guard and preserve explicit approval semantics."""
        environment = dict(self.environment)
        if allow is not None:
            environment['DEPENDENCY_DISCIPLINE_ALLOW'] = allow
        before = (self.root / 'Cargo.lock').read_bytes()
        result = subprocess.run(['bash', 'ci/check_dependency_discipline.sh'], cwd=self.root,
                                env=environment, text=True, capture_output=True)
        self.assertEqual((self.root / 'Cargo.lock').read_bytes(), before)
        self.assertFalse((self.root / 'target').exists(), 'metadata controls must not build Rust')
        return result

    def test_unchanged_workspace_keeps_existing_success(self):
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_valid_local_edge_passes_without_external_approval(self):
        self.add_local_edge()
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('no validated local-edge exception', result.stderr)

    def test_staged_valid_local_edge_uses_current_manifests(self):
        self.add_local_edge()
        self.git('add', 'xtask/Cargo.toml', 'Cargo.lock')
        result = self.check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_local_declaration_without_lock_edge_still_fails(self):
        self.add_local_edge()
        (self.root / 'Cargo.lock').write_text(self.base_lock)
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('xtask/Cargo.toml: fastpq_isi', result.stderr)

    def test_invalid_local_manifest_grants_no_exception(self):
        self.add_local_edge()
        self.append('xtask/Cargo.toml', '\n[features]\ndefault = []\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('local-edge manifest has additional changes', result.stderr)
        self.assertIn('xtask/Cargo.toml: fastpq_isi', result.stderr)

    def test_valid_local_edge_does_not_exempt_same_name_in_other_manifest(self):
        self.add_local_edge()
        self.append('crates/other/Cargo.toml', '\n[dev-dependencies]\nfastpq_isi = { path = "../fastpq_isi" }\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('no validated local-edge exception', result.stderr)
        self.assertIn('crates/other/Cargo.toml: fastpq_isi', result.stderr)
        self.assertNotIn('xtask/Cargo.toml: fastpq_isi', result.stderr)

    def test_valid_local_edge_does_not_exempt_unrelated_external_addition(self):
        self.add_local_edge()
        self.append('crates/other/Cargo.toml', '\n[dev-dependencies]\nexternal_new = "1.0.0"\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn('no validated local-edge exception', result.stderr)
        self.assertIn('crates/other/Cargo.toml: external_new', result.stderr)
        self.assertNotIn('xtask/Cargo.toml: fastpq_isi', result.stderr)

    def test_external_addition_requires_explicit_existing_allow(self):
        self.append('xtask/Cargo.toml', '\n[dependencies]\nexternal_new = "1.0.0"\n')
        refused = self.check()
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn('xtask/Cargo.toml: external_new', refused.stderr)
        allowed = self.check(allow=' irrelevant, external_new ')
        self.assertEqual(allowed.returncode, 0, allowed.stderr)

    def test_helper_refusal_falls_back_to_normal_policy_and_explicit_allow(self):
        self.append('xtask/Cargo.toml', '\n[dependencies]\nexternal_new = "1.0.0"\n')
        (self.root / 'Cargo.lock').write_text(self.base_lock + '\n[[package]]\nname = "external_new"\nversion = "1.0.0"\nsource = "registry+https://example.invalid/index"\nchecksum = "' + 'a' * 64 + '"\n')
        refused = self.check()
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn('lockfile package set, version or source changed', refused.stderr)
        self.assertIn('xtask/Cargo.toml: external_new', refused.stderr)
        allowed = self.check(allow='external_new')
        self.assertEqual(allowed.returncode, 0, allowed.stderr)
        self.assertIn('no validated local-edge exception', allowed.stderr)

    def test_missing_helper_grants_no_implicit_dependency_approval(self):
        self.add_local_edge()
        (self.root / 'scripts/check_agents_local_dependency_edges.py').unlink()
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('no validated local-edge exception', result.stderr)
        self.assertIn('xtask/Cargo.toml: fastpq_isi', result.stderr)

    def test_existing_allow_remains_available_with_valid_local_exception(self):
        self.add_local_edge()
        self.append('crates/other/Cargo.toml', '\n[dev-dependencies]\nexternal_new = "1.0.0"\n')
        result = self.check(allow='external_new')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('no validated local-edge exception', result.stderr)


if __name__ == '__main__':
    unittest.main()
