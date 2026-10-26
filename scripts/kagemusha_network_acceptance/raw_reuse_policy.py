"""Bounded original custody for optional raw DATA reuse, without catalog authority."""
from pathlib import Path
import hashlib
import json
import os
import re
import stat
import sys

from paths import ROOT, QUAL

if sys.flags.optimize:
    raise SystemExit('optimized Python is unsupported')


def require(ok, message):
    if not ok:
        raise ValueError(message)


def pin(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None and value != '0' * 64, 'nonzero lowercase SHA-256 required')
    return value


def identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def bounded(path, cap):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path, 'unaliased absolute original required')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= cap, 'nonempty bounded regular original required: ' + str(path))
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        data = stream.read(cap + 1)
        after = os.fstat(stream.fileno())
    require(len(data) == before.st_size and identity(before) == identity(opened) == identity(after) == identity(path.lstat()), 'original changed while read')
    return data


def sha(path):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path, 'unaliased hash original required')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode), 'regular hash original required')
    digest = hashlib.sha256()
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        for block in iter(lambda: stream.read(1 << 20), b''):
            digest.update(block)
        after = os.fstat(stream.fileno())
    require(identity(before) == identity(opened) == identity(after) == identity(path.lstat()), 'hash input changed')
    return digest.hexdigest()


def doc(path, expected=None):
    data = bounded(path, 32 << 20)
    if expected is not None:
        require(hashlib.sha256(data).hexdigest() == pin(expected), 'selected document hash differs')
    return json.loads(data)


def save(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    return {'path': str(path), 'sha256': sha(path)}
