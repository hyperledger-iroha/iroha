"""Bounded DATA custody utilities extracted unchanged from the retained reader."""
from pathlib import Path
import hashlib
import json
import os
import stat

def identity(s):
    return [s.st_dev, s.st_ino, s.st_mode, s.st_size, s.st_mtime_ns, s.st_ctime_ns]


def directory(path):
    s = path.lstat()
    if not stat.S_ISDIR(s.st_mode):
        raise ValueError(f'not an ordinary directory: {path}')
    # Directory mtime may change for unrelated PK activity. Its custody identity
    # must remain unchanged; exact metadata names and file identities are checked.
    return [s.st_dev, s.st_ino, s.st_mode]


def inspect(path, limit, destination=None):
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or not 0 < before.st_size <= limit:
        raise ValueError(f'metadata shape/size refused: {path}')
    flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0)
    fd = os.open(path, flags)
    writer = None
    try:
        opened = os.fstat(fd)
        if identity(before) != identity(opened):
            raise ValueError('metadata replaced during open')
        if destination is not None:
            writer = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            total += len(chunk)
            if total > limit:
                raise ValueError('metadata grew beyond bound')
            digest.update(chunk)
            if writer is not None:
                view = memoryview(chunk)
                while view:
                    wrote = os.write(writer, view)
                    if wrote <= 0:
                        raise OSError('zero-length output write')
                    view = view[wrote:]
        if total != opened.st_size or identity(opened) != identity(os.fstat(fd)) or identity(opened) != identity(path.lstat()):
            raise ValueError('metadata changed during bounded read')
        if writer is not None:
            os.fchmod(writer, 0o400)
            os.fsync(writer)
        return {'identity': identity(opened), 'bytes': total, 'sha256': digest.hexdigest()}
    finally:
        if writer is not None:
            os.close(writer)
        os.close(fd)


def save(path, data):
    with path.open('xb') as out:
        out.write(data)
        out.flush()
        os.fsync(out.fileno())
    path.chmod(0o400)


def json_file(path, value):
    save(path, (json.dumps(value, sort_keys=True, indent=2) + '\n').encode())


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_DIRECTORY', 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
