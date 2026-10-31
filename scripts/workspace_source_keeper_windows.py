"""Windows original handles for the canonical Python workspace source owner.

Requires native Windows Python and a local fixed NTFS source tree. This internal
module supplies identity, bounded link payloads and permanent mutation latches;
it does not choose a source census, reinterpret a digest or admit private output
permissions. It never invokes Linux, follows a source reparse point, repairs an
ACL, or treats an unsupported filesystem operation as successful custody.
"""

from __future__ import annotations

import ctypes
from ctypes import wintypes as w
import os
from pathlib import Path
import struct


kernel = ctypes.WinDLL("kernel32", use_last_error=True)
security = ctypes.WinDLL("advapi32", use_last_error=True)
INVALID_HANDLE = ctypes.c_void_p(-1).value
GENERIC_READ = 0x80000000
READ_CONTROL = 0x00020000
SHARE_READ = 1
OPEN_EXISTING = 3
OPEN_REPARSE_POINT = 0x00200000
BACKUP_SEMANTICS = 0x02000000
REPARSE_POINT = 0x400
DIRECTORY = 0x10
SYMLINK_TAG = 0xA000000C
GET_REPARSE_POINT = 0x000900A8
MAX_REPARSE_BYTES = 16 * 1024
MAX_SECURITY_BYTES = 64 * 1024


def _function(library, name, arguments, result):
    function = getattr(library, name)
    function.argtypes = arguments
    function.restype = result
    return function


_create = _function(kernel, "CreateFileW", [w.LPCWSTR, w.DWORD, w.DWORD,
                    ctypes.c_void_p, w.DWORD, w.DWORD, w.HANDLE], w.HANDLE)
_close = _function(kernel, "CloseHandle", [w.HANDLE], w.BOOL)
_info = _function(kernel, "GetFileInformationByHandleEx",
                  [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD], w.BOOL)
_device_io = _function(kernel, "DeviceIoControl", [w.HANDLE, w.DWORD,
                      ctypes.c_void_p, w.DWORD, ctypes.c_void_p, w.DWORD,
                      ctypes.POINTER(w.DWORD), ctypes.c_void_p], w.BOOL)
_current_process = _function(kernel, "GetCurrentProcess", [], w.HANDLE)
_duplicate = _function(kernel, "DuplicateHandle", [w.HANDLE, w.HANDLE, w.HANDLE,
                      ctypes.POINTER(w.HANDLE), w.DWORD, w.BOOL, w.DWORD], w.BOOL)
_get_security = _function(security, "GetKernelObjectSecurity", [w.HANDLE, w.DWORD,
                         ctypes.c_void_p, w.DWORD, ctypes.POINTER(w.DWORD)], w.BOOL)
_first_change = _function(kernel, "FindFirstChangeNotificationW",
                         [w.LPCWSTR, w.BOOL, w.DWORD], w.HANDLE)
_close_change = _function(kernel, "FindCloseChangeNotification", [w.HANDLE], w.BOOL)
_wait = _function(kernel, "WaitForSingleObject", [w.HANDLE, w.DWORD], w.DWORD)
_drive_type = _function(kernel, "GetDriveTypeW", [w.LPCWSTR], w.UINT)
_volume = _function(kernel, "GetVolumeInformationByHandleW", [w.HANDLE, w.LPWSTR,
                    w.DWORD, ctypes.POINTER(w.DWORD), ctypes.POINTER(w.DWORD),
                    ctypes.POINTER(w.DWORD), w.LPWSTR, w.DWORD], w.BOOL)


class BasicInfo(ctypes.Structure):
    _fields_ = [(name, ctypes.c_int64) for name in
                ("creation", "access", "write", "change")] + [("attributes", w.DWORD)]


class StandardInfo(ctypes.Structure):
    _fields_ = [("allocation", ctypes.c_int64), ("length", ctypes.c_int64),
                ("links", w.DWORD), ("deleting", ctypes.c_ubyte),
                ("directory", ctypes.c_ubyte)]


class IdentityInfo(ctypes.Structure):
    _fields_ = [("volume", ctypes.c_uint64), ("file_id", ctypes.c_ubyte * 16)]


class TagInfo(ctypes.Structure):
    _fields_ = [("attributes", w.DWORD), ("tag", w.DWORD)]


def _error():
    return ctypes.WinError(ctypes.get_last_error())


def _query(handle, kind, cls):
    result = cls()
    if not _info(handle, kind, ctypes.byref(result), ctypes.sizeof(result)):
        raise _error()
    return result


def _security_snapshot(handle) -> bytes:
    required = w.DWORD()
    success = _get_security(handle, 7, None, 0, ctypes.byref(required))
    if success or ctypes.get_last_error() != 122:
        raise OSError("source security descriptor size query failed")
    if not 0 < required.value <= MAX_SECURITY_BYTES:
        raise OSError("source security descriptor exceeds its bound")
    buffer = ctypes.create_string_buffer(required.value)
    if not _get_security(handle, 7, buffer, len(buffer), ctypes.byref(required)):
        raise _error()
    if required.value > len(buffer):
        raise OSError("source security descriptor changed size")
    return buffer.raw[:required.value]


def _snapshot(handle):
    identity = _query(handle, 18, IdentityInfo)
    basic = _query(handle, 0, BasicInfo)
    standard = _query(handle, 1, StandardInfo)
    tag = _query(handle, 9, TagInfo)
    if (standard.deleting or standard.length < 0 or
            basic.attributes != tag.attributes):
        raise OSError("source original is being deleted or has inconsistent metadata")
    return ((identity.volume, bytes(identity.file_id)), basic.creation,
            basic.write, basic.change, basic.attributes, standard.length,
            standard.links, bool(standard.directory), tag.tag,
            _security_snapshot(handle))


def _open(path: Path):
    value = str(path)
    if "\x00" in value:
        raise ValueError("source path contains NUL")
    handle = _create(value, GENERIC_READ | READ_CONTROL, SHARE_READ, None,
                     OPEN_EXISTING, OPEN_REPARSE_POINT | BACKUP_SEMANTICS, None)
    if handle == INVALID_HANDLE:
        raise _error()
    return handle


def _read_target(handle) -> bytes:
    storage = ctypes.create_string_buffer(MAX_REPARSE_BYTES)
    returned = w.DWORD()
    if not _device_io(handle, GET_REPARSE_POINT, None, 0, storage,
                      len(storage), ctypes.byref(returned), None):
        raise _error()
    if returned.value > len(storage):
        raise OSError("source reparse payload is truncated or exceeds its bound")
    return _decode_target(storage.raw[:returned.value])


def _decode_target(raw: bytes) -> bytes:
    if not 20 <= len(raw) <= MAX_REPARSE_BYTES:
        raise OSError("source reparse payload is truncated or exceeds its bound")
    tag, length, _reserved = struct.unpack_from("<IHH", raw)
    if tag != SYMLINK_TAG or length + 8 != len(raw):
        raise OSError("unsupported source reparse tag or payload length")
    offset, size, print_offset, print_size, flags = struct.unpack_from("<HHHHI", raw, 8)
    payload = raw[20:]
    for start, count in ((offset, size), (print_offset, print_size)):
        if start % 2 or count % 2 or start + count > len(payload):
            raise OSError("source reparse target offsets exceed its payload")
    if flags & ~1:
        raise OSError("source symbolic link has unknown flags")
    target = payload[offset:offset + size].decode("utf-16-le", "strict")
    if not target or "\x00" in target:
        raise OSError("source symbolic link target is empty or contains NUL")
    # Match CPython os.readlink: SubstituteName, with only its NT namespace
    # spelling conversion. PrintName and resolved target bytes are not authority.
    if len(target) > 4 and target.startswith("\\??\\"):
        target = "\\\\?\\" + target[4:]
    return os.fsencode(target)


def require_local_ntfs(root: Path, original) -> None:
    drive = root.drive
    if len(drive) != 2 or drive[1] != ":" or not drive[0].isascii() or not drive[0].isalpha():
        raise OSError("source keeper requires a local drive, not a network namespace")
    if _drive_type(drive + "\\") != 3:
        raise OSError("source keeper requires a fixed local drive")
    filesystem = ctypes.create_unicode_buffer(32)
    if not _volume(original.handle, None, 0, None, None, None,
                   filesystem, len(filesystem)):
        raise _error()
    if filesystem.value != "NTFS":
        raise OSError("source keeper requires native NTFS semantics")


class WindowsOriginal:
    """One original with full native identity and no write/delete sharing."""

    def __init__(self, path: Path, *, ancestor: bool = False):
        self.path = path
        self.ancestor = ancestor
        self.handle = _open(path)
        try:
            self.before = _snapshot(self.handle)
            self.identity = self.before[0]
            attributes, links, directory, tag = (self.before[index] for index in (4, 6, 7, 8))
            if attributes & REPARSE_POINT:
                if ancestor or tag != SYMLINK_TAG or links != 1:
                    raise OSError("source ancestry or member has an unsupported reparse point")
                self.kind = "link"
                self.target = _read_target(self.handle)
            elif directory:
                self.kind, self.target = "directory", None
            else:
                if tag or links != 1:
                    raise OSError("source regular input has indirect or shared identity")
                self.kind, self.target = "file", None
            if ancestor and self.kind != "directory":
                raise OSError("source ancestry is not a real directory")
            self.revalidate()
        except BaseException:
            self.close()
            raise

    def revalidate(self) -> None:
        if self.handle is None:
            raise OSError("source original is already closed")
        held = _snapshot(self.handle)
        named_handle = _open(self.path)
        try:
            named = _snapshot(named_handle)
        finally:
            _close(named_handle)
        if self.ancestor:
            # Outside-source directory contents may change; identity, type and
            # complete original owner/group/DACL must remain exact.
            slots = (0, 4, 7, 8, 9)
            expected = tuple(self.before[index] for index in slots)
            same = (tuple(held[index] for index in slots) == expected ==
                    tuple(named[index] for index in slots))
        else:
            same = held == self.before == named
        if not same:
            raise OSError("source native original or selected name changed")
        if self.kind == "link" and _read_target(self.handle) != self.target:
            raise OSError("original source link target changed")

    def duplicate_reader(self) -> int:
        import msvcrt
        if self.kind != "file":
            raise OSError("source input is not a regular file")
        self.revalidate()
        process = _current_process()
        copied = w.HANDLE()
        if not _duplicate(process, self.handle, process, ctypes.byref(copied), 0, False, 2):
            raise _error()
        try:
            descriptor = msvcrt.open_osfhandle(copied.value, os.O_RDONLY | os.O_BINARY | os.O_NOINHERIT)
        except BaseException:
            _close(copied)
            raise
        try:
            if os.get_inheritable(descriptor):
                raise OSError("source reader unexpectedly permits handle inheritance")
            os.lseek(descriptor, 0, os.SEEK_SET)
        except BaseException:
            os.close(descriptor)
            raise
        return descriptor

    def close(self) -> None:
        if self.handle is not None:
            _close(self.handle)
            self.handle = None


class ChangeLatch:
    """One bounded kernel notification that is never cleared or rearmed."""

    def __init__(self, root: Path, *, recursive: bool):
        # Names, directories, attributes, size, last-write and security. Access
        # time is deliberately absent: ordinary source reads are permitted.
        self.handle = _first_change(str(root), recursive, 0x01 | 0x02 | 0x04 | 0x08 | 0x10 | 0x100)
        if self.handle == INVALID_HANDLE:
            self.handle = None
            raise _error()

    def revalidate(self) -> None:
        if self.handle is None:
            raise OSError("source mutation latch is already closed")
        result = _wait(self.handle, 0)
        if result == 0x102:
            return
        if result == 0xFFFFFFFF:
            raise _error()
        raise OSError("source namespace or security changed during its retained session")

    def close(self) -> None:
        if self.handle is not None:
            _close_change(self.handle)
            self.handle = None


class SecurityLatch(ChangeLatch):
    """Permanent nonrecursive security-only observation of one exact root's parent."""

    def __init__(self, parent: Path):
        self.handle = _first_change(str(parent), False, 0x100)
        if self.handle == INVALID_HANDLE:
            self.handle = None
            raise _error()
