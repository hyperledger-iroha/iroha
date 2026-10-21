"""Current original Git/source custody refuses synthesis and substituted capture values."""
from pathlib import Path
from paths import QUAL
import copy
import json
import tempfile
import unittest
from unittest.mock import patch
import compiler_capture as c


class MetadataAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=QUAL)
        self.assertTrue(Path(self.temp.name).is_relative_to(QUAL))
        self.assertEqual(Path(self.temp.name).stat().st_mode & 0o777, 0o700)
        self.addCleanup(self.temp.cleanup); self.root=Path(self.temp.name).resolve()
        self.patch=patch.object(c,'ROOT',self.root);self.patch.start();self.addCleanup(self.patch.stop)
        names={'.git/HEAD':b'ref: refs/heads/optimizations\n', '.git/refs/heads/optimizations':b'12'*20+b'\n',
               '.git/index':b'actual index', '.git/info/exclude':None,'.git/packed-refs':None}
        self.paths=set(); administration={}
        for name,raw in names.items():
            path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);self.paths.add(path)
            if raw is not None:path.write_bytes(raw)
            administration[name]=None if raw is None else c.sha(path)
        self.before={'head':'12'*20,'branch':'optimizations','unmerged':[],'read_failures':{},
                     'files':{},'links':{},'dependencies':{},'administration':administration}
        self.after=copy.deepcopy(self.before)

    def admit(self):
        c.validate_source_pair(self.before,self.after)
        c.bind_watches(self.paths,self.before,self.after)

    def test_exact_original_values_join_passes(self):self.admit()

    def test_capture_commit_and_branch_substitution_refuse(self):
        for field,value in [('head','34'*20),('branch','foreign')]:
            with self.subTest(field=field):
                self.after=copy.deepcopy(self.before);self.after[field]=value
                with self.assertRaises(ValueError):self.admit()

    def test_unknown_or_missing_git_inventory_refuses(self):
        self.after['administration']['.git/config']='ab'*32
        with self.assertRaisesRegex(ValueError,'inventory'):self.admit()
        self.after=copy.deepcopy(self.before);del self.after['administration']['.git/HEAD']
        with self.assertRaisesRegex(ValueError,'inventory'):self.admit()

    def test_synthesized_build_time_claim_and_forged_current_bytes_refuse(self):
        self.after['raw_build_time_bytes_claimed']=False
        with self.assertRaisesRegex(ValueError,'inventory'):self.admit()
        self.after=copy.deepcopy(self.before);(self.root/'.git/HEAD').write_text('ref: refs/heads/foreign\n')
        with self.assertRaisesRegex(ValueError,'prospective'):self.admit()

    def test_foreign_source_and_added_directive_refuse(self):
        self.before['files']['crates/build-support/script.rs']='12'*32
        self.after=copy.deepcopy(self.before);self.after['files']['crates/build-support/script.rs']='34'*32
        with self.assertRaisesRegex(ValueError,'changed'):self.admit()
        self.after=copy.deepcopy(self.before);self.after['dependencies']['build-directive']='56'*32
        with self.assertRaisesRegex(ValueError,'changed'):self.admit()

    def test_missing_or_mismatched_prospective_original_refuses(self):
        name='.git/refs/heads/optimizations';self.after['administration'][name]='ab'*32
        with self.assertRaisesRegex(ValueError,'changed'):self.admit()
        self.after=copy.deepcopy(self.before);(self.root/name).unlink()
        with self.assertRaisesRegex(ValueError,'absent'):self.admit()

    def test_foreign_retained_path_and_aliased_original_refuse(self):
        original=self.root/'.git/HEAD';other=self.root/'foreign';other.write_bytes(original.read_bytes())
        self.paths.remove(original);self.paths.add(other)
        with self.assertRaisesRegex(ValueError,'prospective'):self.admit()
        self.paths.remove(other);self.paths.add(original);original.unlink();original.symlink_to(other)
        with self.assertRaisesRegex(ValueError,'watched link'):self.admit()

    def test_metadata_absence_cannot_become_empty_valid_closure(self):
        self.after['administration']={}
        with self.assertRaisesRegex(ValueError,'inventory'):self.admit()
        self.after=copy.deepcopy(self.before);self.after['read_failures']={'input':'unavailable'}
        with self.assertRaisesRegex(ValueError,'unresolved'):self.admit()


if __name__=='__main__':unittest.main()
