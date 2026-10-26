"""Pending-merge refusal with inert Git data; never starts build or network work."""
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import capture
import compiler_capture as compiler
import run as controller
from paths import QUAL


class MergeStateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=QUAL)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.git_dir = self.root / '.git'
        self.git_dir.mkdir()
        self.merge = self.git_dir / 'MERGE_HEAD'
        self.responses = {('branch', '--show-current'): 'optimizations',
                          ('diff', '--name-only', '--diff-filter=U'): '',
                          ('rev-parse', 'HEAD'): '1' * 40}
        for module, name, value in ((capture, 'REPO', self.root),
                                    (compiler, 'ROOT', self.root),
                                    (compiler, 'QUAL', self.root / 'target/qualification')):
            guard = patch.object(module, name, value)
            guard.start(); self.addCleanup(guard.stop)
        query = patch.object(capture, 'git', side_effect=lambda *args: self.responses[args])
        self.query = query.start(); self.addCleanup(query.stop)

    def test_resolved_original_records_explicit_merge_absence(self):
        self.assertEqual(capture.require_resolved_checkout(),
                         {'branch': 'optimizations', 'unmerged': [], 'merge_head': False})

    def test_pending_merge_refuses_every_entry_before_followup_or_output(self):
        self.merge.write_text('2' * 40 + '\n')
        output = self.root / 'target/qualification/never-created'
        calls = {
            'capture': capture.snapshot,
            'source': lambda: compiler.source_snapshot({}, 'native', {}),
            'producer': lambda: compiler.producer(output, {}),
            'admission': lambda: compiler.admit({}, set()),
            'stage': lambda: controller.unchanged({}, {}),
        }
        with patch.object(compiler, 'maintained') as helpers, \
             patch.object(compiler, 'tools') as tools, \
             patch.object(compiler, 'metadata') as metadata, \
             patch.object(compiler, 'selection_paths') as selection, \
             patch.object(capture.child_process, 'natural_run') as child:
            for name, call in calls.items():
                with self.subTest(entry=name), self.assertRaisesRegex(ValueError, 'MERGE_HEAD'):
                    call()
            for mock in (self.query, helpers, tools, metadata, selection, child):
                mock.assert_not_called()
        self.assertFalse(output.exists())
        self.assertEqual(list(self.root.iterdir()), [self.git_dir])
        self.merge.unlink()
        self.assertIs(capture.require_resolved_checkout()['merge_head'], False)

    def test_empty_directory_and_dangling_merge_markers_also_refuse(self):
        for kind in ('empty', 'directory', 'dangling'):
            if kind == 'empty': self.merge.touch()
            elif kind == 'directory': self.merge.mkdir()
            else: self.merge.symlink_to(self.root / 'missing')
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, 'MERGE_HEAD'):
                capture.require_resolved_checkout()
            if kind == 'directory': self.merge.rmdir()
            else: self.merge.unlink()
        self.query.assert_not_called()

    def test_branch_and_index_conflicts_refuse(self):
        for branch, conflicts in [('other', ''), ('optimizations', 'unmerged.rs\n')]:
            self.responses[('branch', '--show-current')] = branch
            self.responses[('diff', '--name-only', '--diff-filter=U')] = conflicts
            with self.subTest(branch=branch, conflicts=conflicts), self.assertRaisesRegex(ValueError, 'resolved branch'):
                capture.require_resolved_checkout()

    def test_merge_starting_during_git_observation_refuses(self):
        def query(*args):
            if args[0] == 'diff': self.merge.write_text('2' * 40)
            return self.responses[args]
        self.query.side_effect = query
        with self.assertRaisesRegex(ValueError, 'MERGE_HEAD'):
            capture.require_resolved_checkout()

    def test_merge_starting_during_source_capture_refuses(self):
        def listing(*args, **kwargs):
            self.merge.write_text('2' * 40)
            return SimpleNamespace(stdout=b'')
        with patch.object(capture.child_process, 'natural_run', side_effect=listing) as child, \
             patch.object(capture.source_audit, 'package_paths', return_value=set()):
            with self.assertRaisesRegex(ValueError, 'MERGE_HEAD'):
                capture.snapshot()
        self.assertEqual(child.call_count, 1)

    def test_linked_git_directory_and_shared_administration_refuse(self):
        self.git_dir.rmdir()
        other = self.root / 'other-git'; other.mkdir()
        self.git_dir.symlink_to(other, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'original checkout'):
            capture.require_resolved_checkout()
        self.git_dir.unlink(); self.git_dir.mkdir()
        (self.git_dir / 'commondir').symlink_to(self.root / 'missing')
        with self.assertRaisesRegex(ValueError, 'original checkout'):
            capture.require_resolved_checkout()
        self.query.assert_not_called()

    def test_merge_state_changes_remain_visible_in_source_differences(self):
        before = {'files': {}, 'links': {}, 'head': '1' * 40,
                  'branch': 'optimizations', 'unmerged': [], 'merge_head': False}
        self.assertEqual(capture.changes(before, before)['metadata'], {})
        after = before | {'merge_head': True}
        self.assertEqual(capture.changes(before, after)['metadata'], {'merge_head': [False, True]})


if __name__ == '__main__':
    unittest.main()
