"""Actual same-inode descriptor reuse and flag negatives; no child process."""
import fcntl
import json
import os
from pathlib import Path
import sys
import unittest

from preflight_context import current_context
_CONTEXT = current_context()
BASE = _CONTEXT.work_root
import bootstrap_cases as support
m=support.m

class DescriptorPins(unittest.TestCase):
    setUp=support.DependencyTests.setUp
    tearDown=support.DependencyTests.tearDown
    owner=support.DependencyTests.owner

    def assert_rejected_and_preserved(self,owner,fd):
        try:
            with self.assertRaises(m.ScalingBootstrapError):owner.validate()
            owner.close()
            os.fstat(fd)
        finally:
            try:os.close(fd)
            except OSError:pass

    def test_same_inode_writable_file_reuse(self):
        owner=self.owner()
        held=next(row for row in owner._held if row['path']==self.source/'blake3/__init__.py')
        fd=held['descriptor']; original=os.fstat(fd)
        replacement=os.open(held['path'],os.O_WRONLY|os.O_CLOEXEC)
        os.dup2(replacement,fd,inheritable=False);os.close(replacement)
        self.assertEqual(m._identity(os.fstat(fd)),m._identity(original))
        self.assert_rejected_and_preserved(owner,fd)

    def test_same_inode_inheritable_file_reuse(self):
        owner=self.owner();held=owner._held[0];fd=held['descriptor']
        replacement=os.open(held['path'],os.O_RDONLY|os.O_CLOEXEC)
        os.dup2(replacement,fd,inheritable=True);os.close(replacement)
        self.assert_rejected_and_preserved(owner,fd)

    def test_file_status_flag_change(self):
        owner=self.owner();fd=owner._held[0]['descriptor']
        fcntl.fcntl(fd,fcntl.F_SETFL,fcntl.fcntl(fd,fcntl.F_GETFL)^os.O_NONBLOCK)
        self.assert_rejected_and_preserved(owner,fd)

    def test_same_inode_retained_directory_reuse(self):
        owner=self.owner();row=owner._directories[self.source];fd=row[0]
        replacement=os.open(self.source,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC)
        os.dup2(replacement,fd,inheritable=True);os.close(replacement)
        self.assert_rejected_and_preserved(owner,fd)

    def test_same_inode_held_parent_directory_reuse(self):
        owner=self.owner();held=owner._held[0];fd=held['parent_fd']
        replacement=os.open(held['path'].parent,os.O_RDONLY|os.O_DIRECTORY|os.O_CLOEXEC)
        os.dup2(replacement,fd,inheritable=True);os.close(replacement)
        self.assert_rejected_and_preserved(owner,fd)

    def test_close_alone_preserves_same_inode_writable_slot(self):
        owner=self.owner()
        held=next(row for row in owner._held if row['path']==self.source/'blake3/__init__.py')
        fd=held['descriptor'];replacement=os.open(held['path'],os.O_WRONLY|os.O_CLOEXEC)
        os.dup2(replacement,fd,inheritable=False);os.close(replacement)
        try:
            owner.close();os.fstat(fd)
        finally:
            try:os.close(fd)
            except OSError:pass
