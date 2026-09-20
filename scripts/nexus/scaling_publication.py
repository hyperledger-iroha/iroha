"""Internal bounded publication of one retained physical control inode.

Callers own exact role selection, original parent/source authority and semantic
validation. This helper preserves the reviewed positional-write and no-replace
link/unlink sequence. Its returned descriptor conveys physical ownership only.
"""
from dataclasses import dataclass
import fcntl
import hashlib
import os
import re
import stat
import sys

from resource_bundle import _identity
from resource_evidence_budget import MAX_FILE_BYTES

_WRITE = os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
# Darwin's own write can add FWASWRITTEN; accept only that exact transition.
# https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/fcntl.h
_WRITTEN = 0x00010000 if sys.platform == 'darwin' else 0
_NAME = re.compile(r'[a-z][a-z0-9_.-]{0,127}')
_HEX = re.compile(r'[0-9a-f]{64}')


class PublicationError(ValueError):
    """Internal physical publication failure with no path or byte disclosure."""


def _require(value):
    if not value: raise PublicationError('publication_failed')


@dataclass(frozen=True, slots=True)
class CreatedFile:
    """One newly owned original descriptor returned after complete publication."""
    fd: int
    identity: tuple
    flags: int


def _close_owned(fd, inode):
    """Do not close a foreign descriptor which reused an owned integer."""
    try:
        if _identity(os.fstat(fd))[:2] == inode:
            os.close(fd)
    except OSError:
        pass


def _flags(fd, *, readonly):
    _require(type(fd) is int and fd >= 0 and not os.get_inheritable(fd))
    value = fcntl.fcntl(fd, fcntl.F_GETFL)
    _require(value & os.O_ACCMODE == (os.O_RDONLY if readonly else os.O_RDWR))
    return value


def _file_identity(fd, cap, *, readonly):
    flags = _flags(fd, readonly=readonly)
    info = os.fstat(fd)
    _require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid()
             and stat.S_IMODE(info.st_mode) == 0o600 and info.st_nlink == 1
             and 0 < info.st_size <= cap)
    return _identity(info), flags


def names(fd, cap):
    names = []
    with os.scandir(fd) as entries:
        for entry in entries:
            _require(len(names) < cap)
            names.append(entry.name)
    return tuple(sorted(names))


def digest_file(fd, identity, guard):
    digest, offset = hashlib.sha256(), 0
    while offset < identity[6]:
        guard()
        _require(_identity(os.fstat(fd)) == identity)
        count = min(65536, identity[6] - offset)
        raw = os.pread(fd, count, offset)
        _require(0 < len(raw) <= count)
        digest.update(raw)
        offset += len(raw)
        guard()
    _require(_identity(os.fstat(fd)) == identity)
    return digest.hexdigest()


def publish_file(parent, name, size, max_bytes, expected_sha, read_chunk, check_source, guard, expected_names):
    """Publish one exact bounded inode and transfer only its created descriptor."""
    _require(type(parent) is int and parent >= 0 and type(name) is str and _NAME.fullmatch(name)
             and type(size) is int and type(max_bytes) is int and 0 < size <= max_bytes <= MAX_FILE_BYTES
             and type(expected_sha) is str and _HEX.fullmatch(expected_sha)
             and callable(read_chunk) and callable(check_source) and callable(guard)
             and type(expected_names) is frozenset and len(expected_names) <= 17
             and all(type(item) is str and _NAME.fullmatch(item) for item in expected_names))
    stage = name + '.publishing'
    _require(set(names(parent, 17)) == expected_names)
    fd, owned = None, None
    try:
        fd = os.open(stage, _WRITE, 0o600, dir_fd=parent)
        info = os.fstat(fd)
        owned = _identity(info)[:2]
        _require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                 and stat.S_IMODE(info.st_mode) == 0o600 and info.st_uid == os.geteuid())
        stage_state, stage_flags = _identity(info), _flags(fd, readonly=False)
        def check_stage(expected):
            _require(_identity(os.fstat(fd)) == expected
                     and _flags(fd, readonly=False) == stage_flags
                     and _identity(os.stat(stage, dir_fd=parent, follow_symlinks=False)) == expected)
        accepted, offset = hashlib.sha256(), 0
        while offset < size:
            guard(); check_source()
            check_stage(stage_state)
            count = min(65536, size - offset)
            raw = read_chunk(offset, count)
            _require(type(raw) is bytes and 0 < len(raw) <= count)
            position = 0
            while position < len(raw):
                guard(); check_source()
                check_stage(stage_state)
                # A borrowed open-file description can have its cursor
                # moved without changing inode metadata. Use the counted
                # accepted offset so that cannot create an oversized hole.
                written = os.pwrite(fd, raw[position:], offset + position)
                _require(type(written) is int and 0 < written <= len(raw) - position)
                current = os.fstat(fd)
                _require(_identity(current)[:6] == stage_state[:6]
                         and current.st_size == stage_state[6] + written)
                current_flags = _flags(fd, readonly=False)
                _require(current_flags in (stage_flags, stage_flags | _WRITTEN))
                stage_flags = current_flags
                stage_state = _identity(current)
                check_stage(stage_state)
                accepted.update(raw[position:position + written])
                position += written
            offset += len(raw)
            _require(offset <= max_bytes)
        _require(accepted.hexdigest() == expected_sha)
        check_stage(stage_state)
        os.fsync(fd)
        check_stage(stage_state)
        identity, _ = _file_identity(fd, max_bytes, readonly=False)
        _require(identity[:2] == owned and identity[6] == size
                 and _identity(os.stat(stage, dir_fd=parent, follow_symlinks=False)) == identity)
        _require(digest_file(fd, identity, guard) == expected_sha)
        guard(); check_source()
        _require(set(names(parent, 18)) == expected_names | {stage})
        check_stage(stage_state)
        # Link is the no-replace publication step. Both temporary names name
        # one reserved inode; no final is overwritten. Remove only our stage.
        os.link(stage, name, src_dir_fd=parent, dst_dir_fd=parent, follow_symlinks=False)
        linked = _identity(os.fstat(fd))
        _require(linked[:2] == owned and linked[5] == 2
                 and _identity(os.stat(stage, dir_fd=parent, follow_symlinks=False)) == linked
                 and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == linked)
        os.fsync(parent)
        # fsync is an external boundary too. Do not delete a substituted
        # named stage, even when the still-open created inode is intact.
        check_stage(linked)
        _require(_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == linked)
        os.unlink(stage, dir_fd=parent)
        os.fsync(parent)
        identity, _ = _file_identity(fd, max_bytes, readonly=False)
        _require(identity[:2] == owned and identity[6] == size
                 and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity
                 and set(names(parent, 17)) == expected_names | {name})
        after = _identity(os.fstat(parent))
        _require(digest_file(fd, identity, guard) == expected_sha)
        guard(); check_source()
        _require(_identity(os.fstat(parent)) == after
                 and _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
        result = CreatedFile(fd, identity, _flags(fd, readonly=False))
        fd = None
        return result
    finally:
        if fd is not None and owned is not None: _close_owned(fd, owned)
