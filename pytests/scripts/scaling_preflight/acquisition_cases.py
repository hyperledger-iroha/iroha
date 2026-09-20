"""Actual retained-file acquisition/cleanup boundaries; no child process."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from preflight_context import current_context
_CONTEXT = current_context()
BASE = _CONTEXT.work_root
import copy_sumeragi_v2_release_cargo_cache as m

class Acquisition(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(dir=BASE);self.root=Path(self.temp.name).resolve()
        self.path=self.root/'input';self.path.write_bytes(b'original');self.path.chmod(0o600)
        self.held=[];self.foreign=[]
    def tearDown(self):
        for held in self.held:
            try:m._close_held_regular(held)
            except OSError:pass
        for fd in self.foreign:
            try:os.close(fd)
            except OSError:pass
        self.temp.cleanup()
    def hold(self):
        held=m._hold_regular(self.path,'fixture',maximum_bytes=64);self.held.append(held);return held
    def swap(self,fd,path=None,flags=os.O_WRONLY,inheritable=False):
        other=os.open(path or self.path,flags|os.O_CLOEXEC)
        os.dup2(other,fd,inheritable=inheritable);os.close(other);self.foreign.append(fd)
    def test_original_reader_and_parent_close(self):
        held=self.hold();self.assertEqual(held['data'],b'original');m._revalidate_held_regular(held)
        m._close_held_regular(held)
        for fd in (held['descriptor'],held['parent_fd']):
            with self.assertRaises(OSError):os.fstat(fd)
    def test_read_error_preserves_same_inode_writable_replacement(self):
        slots=[]
        def read(fd,size):
            slots.append(fd);self.swap(fd);raise OSError('private failure data')
        with patch.object(os,'read',read):
            with self.assertRaises(OSError):self.hold()
        os.fstat(slots[0])
    def test_read_error_without_reuse_closes_original_reader_and_parent(self):
        readers=[];parents=[];actual=m._open_directory
        def opened(*args,**kwargs):
            fd,pin=actual(*args,**kwargs);parents.append(fd);return fd,pin
        def read(fd,size):readers.append(fd);raise OSError('read failed')
        with patch.object(m,'_open_directory',opened),patch.object(os,'read',read):
            with self.assertRaises(OSError):self.hold()
        for fd in (readers[0],parents[0]):
            with self.assertRaises(OSError):os.fstat(fd)
    def test_inheritable_reuse_during_successful_read_rejected_and_preserved(self):
        original=os.read;done=[]
        def read(fd,size):
            raw=original(fd,size)
            if not done:
                done.append(fd);self.swap(fd,flags=os.O_RDONLY,inheritable=True)
                os.lseek(fd,len(raw),os.SEEK_SET)
            return raw
        with patch.object(os,'read',read):
            with self.assertRaises(m.CacheCopyError):self.hold()
        os.fstat(done[0])
    def test_parent_reuse_during_read_error_preserved(self):
        parents=[];actual=m._open_directory
        def opened(*args,**kwargs):
            result=actual(*args,**kwargs);parents.append(result[0]);return result
        def read(fd,size):
            self.swap(parents[0],self.root,os.O_RDONLY|os.O_DIRECTORY,True)
            raise OSError('read failed')
        with patch.object(m,'_open_directory',opened),patch.object(os,'read',read):
            with self.assertRaises(OSError):self.hold()
        os.fstat(parents[0])
    def test_revalidation_error_preserves_original_acquisition_pin(self):
        held=self.hold();fd=held['descriptor']
        def read(descriptor,size):
            self.swap(descriptor);raise OSError('recheck failed')
        with patch.object(os,'read',read):
            with self.assertRaises(OSError):m._revalidate_held_regular(held)
        m._close_held_regular(held);os.fstat(fd)
    def test_revalidation_same_inode_inheritance_change_rejected(self):
        held=self.hold();os.set_inheritable(held['descriptor'],True);self.foreign.append(held['descriptor'])
        with self.assertRaises(m.CacheCopyError):m._revalidate_held_regular(held)
        m._close_held_regular(held);os.fstat(held['descriptor'])
    def test_final_revalidation_failure_closes_original_reader(self):
        captured=[];actual=m._revalidate_held_regular
        def failed(held):captured.append(held);raise m.CacheCopyError('final read failed')
        with patch.object(m,'_revalidate_held_regular',failed):
            with self.assertRaises(m.CacheCopyError):self.hold()
        for fd in (captured[0]['descriptor'],captured[0]['parent_fd']):
            with self.assertRaises(OSError):os.fstat(fd)
