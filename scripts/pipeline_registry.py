"""Draft reviewed-module registry, kept alive through a structured pipeline operation.

Initial reads authenticate content against independently reviewed source literals;
only the later canonical keeper admission establishes local filesystem originals,
which remain held before any process launch. Initial hash-authenticated code bytes
do not claim continuous identity custody back through their first content read.
This contains no private ACL creation, repair, release manifest or source census.
"""
from contextlib import ExitStack
import ctypes
import hashlib
import os
from pathlib import Path
import re
import stat
import sys
import types

_MAXIMUM = 2 * 1024 * 1024


def _capture(path):
    """Bounded no-follow code read; no code executes before its review digest matches."""
    if os.name != 'nt':
        fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
        with os.fdopen(fd, 'rb') as stream:
            before = os.fstat(stream.fileno())
            if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_mode & 0o022
                    or not 0 < before.st_size <= _MAXIMUM):
                raise RuntimeError('reviewed bootstrap input is not an admissible regular file')
            raw = stream.read(_MAXIMUM + 1)
            after = os.fstat(stream.fileno())
            fields = lambda info: (info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
                                   info.st_size, info.st_mtime_ns, info.st_ctime_ns)
            if len(raw) != before.st_size or fields(before) != fields(after):
                raise RuntimeError('reviewed bootstrap input changed while reading')
            return raw
    # Minimal content bootstrap only: use the already-trusted operating-system
    # reader to obtain reviewed keeper code without following a reparse or FIFO.
    # Full128 identities/security/ancestor custody belong to that canonical keeper.
    from ctypes import wintypes as w
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    def function(name, args, result):
        value = getattr(kernel, name); value.argtypes = args; value.restype = result
        return value
    opened = function('CreateFileW', [w.LPCWSTR,w.DWORD,w.DWORD,ctypes.c_void_p,w.DWORD,w.DWORD,w.HANDLE], w.HANDLE)
    close = function('CloseHandle', [w.HANDLE], w.BOOL)
    kind = function('GetFileType', [w.HANDLE], w.DWORD)
    info = function('GetFileInformationByHandleEx', [w.HANDLE,ctypes.c_int,ctypes.c_void_p,w.DWORD], w.BOOL)
    length = function('GetFileSizeEx', [w.HANDLE,ctypes.POINTER(ctypes.c_longlong)], w.BOOL)
    read = function('ReadFile', [w.HANDLE,ctypes.c_void_p,w.DWORD,ctypes.POINTER(w.DWORD),ctypes.c_void_p], w.BOOL)
    handle = opened(str(path), 0x80000000, 1, None, 3, 0x00200000 | 0x02000000, None)
    if handle == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        attributes = (w.DWORD * 2)()
        size = ctypes.c_longlong()
        if kind(handle) != 1 or not info(handle, 9, attributes, ctypes.sizeof(attributes)):
            raise RuntimeError('reviewed bootstrap input is not native disk data')
        if attributes[0] & (0x10 | 0x400) or not length(handle, ctypes.byref(size)) or not 0 < size.value <= _MAXIMUM:
            raise RuntimeError('reviewed bootstrap input is indirect or excessive')
        raw = bytearray()
        buffer = ctypes.create_string_buffer(65536)
        while True:
            count = w.DWORD()
            if not read(handle, buffer, len(buffer), ctypes.byref(count), None):
                raise ctypes.WinError(ctypes.get_last_error())
            if count.value == 0: break
            raw.extend(buffer.raw[:count.value])
            if len(raw) > _MAXIMUM:
                raise RuntimeError('reviewed bootstrap input exceeds its bound')
        if len(raw) != size.value:
            raise RuntimeError('reviewed bootstrap input changed extent')
        return bytes(raw)
    finally:
        close(handle)


class ReviewedRegistry:
    """Authenticate all bytes, retain originals, then execute only the captured set."""
    def __init__(self, directory, expected):
        self.directory = Path(directory)
        self.expected = dict(expected)
        self.modules = {}
        self.payloads = {}
        self.stack = ExitStack()
        self.keeper = None

    def _execute(self, name):
        if name in sys.modules:
            raise RuntimeError('a bootstrap module was already installed outside this reviewed registry')
        module = types.ModuleType(name)
        module.__file__ = str(self.directory / (name + '.py'))
        module.__package__ = ''
        sys.modules[name] = module
        self.stack.callback(lambda: sys.modules.pop(name, None) if sys.modules.get(name) is module else None)
        exec(compile(self.payloads[name + '.py'], module.__file__, 'exec'), module.__dict__)
        self.modules[name] = module
        return module

    def __enter__(self):
        try:
            for filename, digest in self.expected.items():
                if (re.fullmatch(r'[a-z_-]+\.(py|json)', filename) is None
                        or re.fullmatch(r'[0-9a-f]{64}', digest) is None):
                    raise RuntimeError('reviewed registry has an invalid static entry')
                raw = _capture(self.directory / filename)
                if hashlib.sha256(raw).hexdigest() != digest:
                    raise RuntimeError('bootstrap bytes differ from the reviewed pipeline source')
                self.payloads[filename] = raw
            native = self._execute('workspace_source_keeper_windows') if os.name == 'nt' else None
            custody = self._execute('workspace_source_keeper')
            self.keeper = self.stack.enter_context(custody.SourceKeeper(self.directory, maximum_members=len(self.expected), native_module=native))
            self.keeper.capture(self.expected)
            if os.name != 'nt':
                # Preserve the existing pipeline's non-shared-write root/file policy.
                for original in self.keeper.originals.values():
                    if (original.path == self.directory or original.path == self.directory.parent
                            or original.path in self.keeper.members) and original.before[2] & 0o022:
                        raise RuntimeError('reviewed bootstrap ancestry or member is shared-writable')
            for filename, expected in self.payloads.items():
                with self.keeper.reader(self.directory / filename) as reader:
                    if reader.read(_MAXIMUM + 1) != expected:
                        raise RuntimeError('retained registry original differs from authenticated bytes')
            for name in ('compute_workspace_source_manifest', 'sumeragi_prebuilt_bundle', 'release_entry_owner',
                         'release_process_bridge', 'release_bridge_admission'):
                self._execute(name)
            self.keeper.revalidate()
            return self
        except BaseException:
            self.stack.close()
            raise

    def prepare(self, invocation, pins, *, bridge_root=None, bridge_pins=None):
        """Keep registry, source, entry and native module in one transferable owner."""
        if (bridge_root is None) != (bridge_pins is None):
            raise RuntimeError('native bridge trust inputs are incomplete')
        if bridge_root is None:
            raise RuntimeError('first entry requires independently pinned native bridge inputs')
        entry = self.modules['release_entry_owner']
        custody = self.modules['workspace_source_keeper']
        native = self.modules.get('workspace_source_keeper_windows')
        bridge_factory=None
        if bridge_root is not None:
            bridge_root=entry._path(str(bridge_root))
            bridge_owner=self.stack.enter_context(entry.RootSecurity(bridge_root,custody,native_module=native))
            for path in (invocation.repo_root,invocation.entry_root,invocation.cargo_root,invocation.artifact_root):
                other=self.stack.enter_context(entry.RootSecurity(path,custody,native_module=native))
                if bridge_owner.identity in other.ancestry or other.identity in bridge_owner.ancestry:
                    raise RuntimeError('native bridge physically overlaps source, entry or output roots')
            def bridge_factory(source,owners):
                bridge_owner.revalidate()
                return self.modules['release_bridge_admission'].load_original(bridge_root,bridge_pins,source,owners,
                    entry_module=entry,custody_module=custody,native_module=native,
                    environment_policy=self.payloads['environment-policy.json'])
        self.keeper.revalidate()
        result = entry.prepare_sumeragi_programs(invocation, pins,
            source_module=self.modules['compute_workspace_source_manifest'],
            custody_module=custody,native_module=native,owners=self.stack,bridge_factory=bridge_factory,
            environment_policy=self.payloads['environment-policy.json'])
        self.keeper.revalidate()
        return result

    def __exit__(self, kind, error, traceback):
        return self.stack.__exit__(kind, error, traceback)
