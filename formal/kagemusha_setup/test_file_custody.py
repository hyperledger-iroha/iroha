"""Small file custody controls migrated from the retired large constructor."""
from dataclasses import replace
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from . import file_custody as subject
from .custody import sha


class FileCustodyTests(unittest.TestCase):
    def temporary(self):
        result=tempfile.TemporaryDirectory(prefix='formal-file-custody-')
        self.addCleanup(result.cleanup)
        return Path(result.name).resolve()

    def test_output_preflight_before_creating_files(self):
        root=self.temporary()
        for limits in (replace(subject.Limits(),output_bytes=0),
                       replace(subject.Limits(),output_bytes=True),
                       replace(subject.Limits(),files=0),replace(subject.Limits(),depth=0)):
            with self.assertRaisesRegex(ValueError,'finite output'):
                subject.Outputs(root/'absent',limits)
            self.assertFalse((root/'absent').exists())
        with self.assertRaisesRegex(ValueError,'fresh output'):
            subject.Outputs(root,subject.Limits())
        with self.assertRaisesRegex(ValueError,'fresh output'):
            subject.Outputs(root/'missing'/'new',subject.Limits())
        link=root/'linked';link.symlink_to(root,target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'symbolic links'):
            subject.Outputs(link/'new',subject.Limits())
        file=root/'file';file.write_bytes(b'DATA')
        with self.assertRaisesRegex(ValueError,'fresh output'):
            subject.Outputs(file/'new',subject.Limits())
        with self.assertRaisesRegex(ValueError,'absolute bounded path'):
            subject.safe(Path('relative'))
        with self.assertRaisesRegex(ValueError,'absolute bounded path'):
            subject.safe(root/'..'/'invalid')

    def test_bound_extent_and_full_tail_required(self):
        root=self.temporary();raw=b'bounded-original';path=root/'small';path.write_bytes(raw)
        pin=(len(raw),sha(raw));observation={}
        self.assertEqual(subject.read_original(path,pin,observation=observation),raw)
        self.assertEqual(observation['bytes_hashed'],len(raw));self.assertEqual(observation['bytes_read'],len(raw))
        self.assertEqual(observation['observed_sha256'],pin[1]);self.assertEqual(observation['extension_sentinel_bytes'],0)
        self.assertTrue(observation['passed']);self.assertTrue(observation['identities_unchanged'])
        self.assertIn('stat_identity_before',observation)
        for changed,why in ((raw[:-1],'extent/type'),(raw+b'x','extent/type'),(b'X'+raw[1:],'exact original digest')):
            path.write_bytes(changed)
            observed={}
            with self.assertRaisesRegex(ValueError,why):subject.read_original(path,pin,observation=observed)
            self.assertFalse(observed['passed']);self.assertEqual(observed['observed_namespace_bytes'],len(changed))
            self.assertIn('error',observed)
            if why=='exact original digest':self.assertEqual(observed['observed_sha256'],sha(changed))
        with self.assertRaisesRegex(ValueError,'bounded original'):subject.read_original(path,(subject.MAX_READ_BYTES+1,'0'*64))

    def test_nofollow_and_namespace_replacement(self):
        root=self.temporary();raw=b'custody';path=root/'original';path.write_bytes(raw)
        link=root/'link';link.symlink_to(path)
        with self.assertRaisesRegex(ValueError,'symbolic'):subject.read_original(link,(len(raw),sha(raw)))
        parent_link=root/'parent-link';parent_link.symlink_to(root,target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'symbolic'):
            subject.read_original(parent_link/'original',(len(raw),sha(raw)))
        replaced=False
        def replace_inode():
            nonlocal replaced
            if not replaced:
                replaced=True;replacement=root/'replacement';replacement.write_bytes(raw);replacement.replace(path)
        with self.assertRaisesRegex(ValueError,'changed during read'):
            subject.read_original(path,(len(raw),sha(raw)),replace_inode)
        self.assertTrue(replaced)
        changed=False
        def change_bytes():
            nonlocal changed
            if not changed:changed=True;path.write_bytes(b'X'+raw[1:])
        with self.assertRaisesRegex(ValueError,'changed during read'):
            subject.read_original(path,(len(raw),sha(raw)),change_bytes)

    def test_charged_partial_writes_and_exact_namespace(self):
        root=self.temporary()
        out=subject.Outputs(root/'bounded',subject.Limits(output_bytes=10,files=1))
        with self.assertRaisesRegex(ValueError,'before write'):
            with out.stream(out.root/'file') as emit:
                emit(b'1234567');emit(b'8901')
        self.assertEqual((out.root/'file').read_bytes(),b'1234567')
        self.assertEqual(out.used,7)
        self.assertEqual(out.records['file']['bytes'],7)
        self.assertEqual(out.verify(),[out.records['file']])
        with self.assertRaisesRegex(ValueError,'file budget'):
            with out.stream(out.root/'second'):pass
        (out.root/'empty-foreign').mkdir()
        with self.assertRaisesRegex(ValueError,'unexpected output directory'):out.verify()

    def test_output_depth_fresh_names_and_live_writer(self):
        out=subject.Outputs(self.temporary()/'bounded',subject.Limits(depth=1))
        with self.assertRaisesRegex(ValueError,'output depth'):
            out.write(out.root/'nested'/'file',b'data')
        self.assertFalse((out.root/'nested').exists())
        with out.stream(out.root/'one') as emit:
            emit(b'data')
            with self.assertRaisesRegex(ValueError,'partial live writer'):out.verify()
            with self.assertRaisesRegex(ValueError,'fresh output name'):
                with out.stream(out.root/'one'):pass
        with self.assertRaisesRegex(ValueError,'fresh output name'):out.write(out.root/'one',b'again')
        out.save(out.root/'record.json',{'small':'DATA'})
        self.assertEqual(len(out.verify()),2)
        self.assertEqual(out.used,len(b'data')+(out.root/'record.json').stat().st_size)

    def test_output_integrity_and_partial_observation(self):
        root=self.temporary()
        for name,changed,reason in (('extent',b'longer','extent'),('hash',b'evil','hash/identity')):
            out=subject.Outputs(root/name,subject.Limits())
            out.write(out.root/'one',b'data');(out.root/'one').write_bytes(changed)
            with self.assertRaisesRegex(ValueError,reason):out.verify()
        for name in ('removed-empty','replaced-empty'):
            out=subject.Outputs(root/name,subject.Limits())
            self.assertEqual(out.verify(),[])
            if name=='removed-empty':out.root.rmdir()
            else:
                out.root.rename(root/'prior-empty')
                out.root.mkdir(mode=0o700)
            with self.assertRaisesRegex(ValueError,'output root changed'):out.verify()
        unreadable=subject.Outputs(root/'unreadable-empty',subject.Limits())
        def failed_walk(directory,*,followlinks,onerror):
            self.assertEqual(directory,unreadable.root)
            self.assertFalse(followlinks)
            onerror(PermissionError('enumeration unavailable'))
            yield directory,[],[]
        with patch.object(subject.os,'walk',failed_walk):
            with self.assertRaisesRegex(PermissionError,'enumeration unavailable'):
                unreadable.verify()
        path=root/'read';path.write_bytes(b'bounded')
        observation={}
        def interrupt():raise KeyboardInterrupt('retained read interrupted')
        with self.assertRaises(KeyboardInterrupt):
            subject.read_original(path,(7,sha(b'bounded')),interrupt,observation=observation)
        self.assertFalse(observation['passed'])
        self.assertEqual(observation['bytes_read'],7)
        self.assertEqual(observation['observed_sha256'],sha(b'bounded'))
        self.assertIn('KeyboardInterrupt',observation['error'])
        for bad in ('G'*64,'0'*63):
            with self.assertRaisesRegex(ValueError,'bounded original'):
                subject.read_original(path,(7,bad))
