"""Clone-only immutable DATA transport; current Rust source admission remains mandatory.

No hardlink, source chmod, old admission grant, or full-copy fallback is provided.
The fclonefileat mechanics match the maintained taira_release_check.py helper.
"""
from pathlib import Path
import ctypes
import hashlib
import fcntl
import os
import shutil
import stat
import sys
import native_data as data

OUTPUT_BYTES = 128 << 30
FILE_BYTES = 1 << 30
RESERVE_BYTES = 20 << 30  # Explicit engineering headroom; not a filesystem reservation.
SELECTION_SCHEMA = 'iroha.kagemusha.sealed-proving-originals-selection.v1'
MEMBERSHIP_SCHEMA = 'iroha.kagemusha.engineering-qualified-source-membership.v1'
LOCATOR_SCHEMA = 'iroha.kagemusha.sealed-proving-originals-locator.v1'
require = data.require


def capacity(path):
    """Cap fresh raw writes without assuming all historical raw entries are reused."""
    free = shutil.disk_usage(path).free
    require(free >= RESERVE_BYTES, 'insufficient disk headroom for clone-only campaign')
    return {'free_bytes': free, 'reserve_bytes': RESERVE_BYTES,
            'maximum_new_raw_bytes': min(OUTPUT_BYTES, free - RESERVE_BYTES),
            'full_copy_fallback': False,
            'scope': 'Fresh observation, not reserved space; concurrent writers remain a launch constraint.'}


def clone_function():
    if sys.platform.startswith('linux'):
        def clone(source, destination, name):
            require(type(name) is str and data.pin(name) == name, 'exact content address required')
            copied = os.open(name, os.O_CREAT | os.O_EXCL | os.O_WRONLY | os.O_NOFOLLOW,
                             0o600, dir_fd=destination)
            try:
                # Linux FICLONE copies into a new independent inode. Unsupported
                # filesystem/device or capacity failure is final: no copy fallback.
                fcntl.ioctl(copied, 0x40049409, source)
            finally:
                os.close(copied)
        return clone
    require(sys.platform == 'darwin', 'clone-only transport requires Linux FICLONE or macOS fclonefileat')
    library = ctypes.CDLL(None, use_errno=True)
    try:
        native = library.fclonefileat
    except AttributeError as error:
        raise ValueError('clone-only transport is unavailable') from error
    native.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]
    native.restype = ctypes.c_int

    def clone(source, destination, name):
        require(type(name) is str and data.pin(name) == name, 'exact content address required')
        if native(source, destination, os.fsencode(name), 0x0001 | 0x0002) == 0:
            return
        code = ctypes.get_errno()
        # ENOTSUP/EXDEV/ENOSYS, ENOSPC and all other failures are final; no copy fallback.
        raise OSError(code, os.strerror(code), name)
    return clone


def identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def rows(value):
    require(type(value) is dict and set(value) == {'schema', 'scope', 'wallet_originals', 'wallet_unique_bytes'}
            and value['schema'] == MEMBERSHIP_SCHEMA and type(value['scope']) is str,
            'exact current membership DATA required')
    result = value['wallet_originals']
    require(type(result) is list and 0 < len(result) <= 4096 * 3, 'bounded membership required')
    names = set(); total = 0
    for row in result:
        require(type(row) is dict and set(row) == {'sha256', 'bytes'}, 'exact membership row required')
        name = data.pin(row['sha256'])
        require(name not in names and type(row['bytes']) is int and 0 < row['bytes'] <= FILE_BYTES,
                'duplicate or oversized membership original')
        names.add(name); total += row['bytes']
    require(type(value['wallet_unique_bytes']) is int and total == value['wallet_unique_bytes']
            and total <= OUTPUT_BYTES, 'membership byte accounting differs')
    return result


def clone_one(source, destination_fd, row, clone):
    """Clone the exact opened original, then seal and hash only the independent copy."""
    before = source.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
            and not before.st_mode & 0o022 and before.st_size == row['bytes'],
            'raw original type/owner/extent differs')
    fd = os.open(source, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        require(identity(os.fstat(fd)) == identity(before), 'raw original changed during open')
        clone(fd, destination_fd, row['sha256'])
        copied = os.open(row['sha256'], os.O_RDONLY | os.O_NOFOLLOW, dir_fd=destination_fd)
        try:
            initial = os.fstat(copied)
            require(stat.S_ISREG(initial.st_mode) and initial.st_nlink == 1
                    and initial.st_uid == os.geteuid() and initial.st_dev == before.st_dev
                    and initial.st_ino != before.st_ino and initial.st_size == row['bytes'],
                    'clone must be a fresh independent inode, never a hardlink or symlink')
            os.fchmod(copied, 0o400)
            sealed = os.fstat(copied)
            digest = hashlib.sha256(); remaining = row['bytes']
            while remaining:
                chunk = os.read(copied, min(65536, remaining))
                require(bool(chunk), 'cloned original truncated')
                remaining -= len(chunk); digest.update(chunk)
            require(os.read(copied, 1) == b'' and digest.hexdigest() == row['sha256'],
                    'cloned original content hash/extent differs')
            require(identity(os.fstat(copied)) == identity(sealed)
                    == identity(os.stat(row['sha256'], dir_fd=destination_fd, follow_symlinks=False)),
                    'cloned original changed during verification')
            os.fsync(copied)
        finally:
            os.close(copied)
        require(identity(os.fstat(fd)) == identity(before) == identity(source.lstat()),
                'raw original changed during clone; source is never hardened in place')
        return {'sha256': row['sha256'], 'bytes': row['bytes'],
                'source_identity': list(identity(before)), 'clone_identity': list(identity(sealed))}
    finally:
        os.close(fd)


def prepare(catalog, membership_ref, pins, output):
    """Fresh private clone DATA with an explicit current-source selection for first TARGET."""
    require(type(pins) is dict and set(pins) == set(data.SOURCE_PINS), 'exact seven current source pins')
    for pin in pins.values():
        data.pin(pin)
    catalog = data.local(catalog); output = data.local(output)
    source_root = data.local(catalog / 'originals')
    require(data.selected(membership_ref, 16 << 20) == catalog / 'qualified-source-membership.json',
            'membership must be the selected current catalog output')
    selected = rows(data.document(membership_ref, 16 << 20))
    require(not output.exists() and not output.is_relative_to(source_root)
            and not source_root.is_relative_to(output), 'fresh independent clone namespace required')
    clone = clone_function()  # Refuse unsupported API before any transport output.
    before_capacity = capacity(output.parent)
    os.mkdir(output, 0o700)
    target = output / 'originals'; os.mkdir(target, 0o700)
    directory = os.open(target, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        original_dir = os.fstat(directory)
        require(stat.S_IMODE(original_dir.st_mode) == 0o700 and original_dir.st_uid == os.geteuid(),
                'exact private clone directory required')
        copied = []
        for row in selected:
            capacity(output)  # Retain failure and prior clones if another writer consumes headroom.
            copied.append(clone_one(source_root / row['sha256'], directory, row, clone))
        os.fsync(directory)
        require({item.name for item in target.iterdir()} == {row['sha256'] for row in selected},
                'clone namespace differs from current membership')
        for row in copied:
            require(list(identity((target / row['sha256']).lstat())) == row['clone_identity'],
                    'sealed clone changed before selection publication')
        require((original_dir.st_dev, original_dir.st_ino) == (target.stat().st_dev, target.stat().st_ino),
                'clone directory replaced')
    finally:
        os.close(directory)
    data.selected(membership_ref, 16 << 20)
    after_capacity = capacity(output)
    selection = {'schema': SELECTION_SCHEMA, 'source_pins': pins,
                 'sealed_originals': {'schema': LOCATOR_SCHEMA, 'path': str(target)}}
    receipt = {'membership': membership_ref, 'source_pins': pins,
               'originals': copied, 'before_capacity': before_capacity, 'after_capacity': after_capacity,
               'scope': 'Independent clone DATA only; complete current Rust source requalification is still required.'}
    return selection, receipt


def probe(output):
    """Require a real tiny same-filesystem clone before launching the costly campaign."""
    output = data.local(output)
    require(not output.exists(), 'fresh clone probe namespace required')
    clone = clone_function()
    os.mkdir(output, 0o700)
    body = b'Untrusted independent clone DATA probe, never source authority.\n'
    source = output / 'probe-source'
    with source.open('xb') as stream:
        stream.write(body); stream.flush(); os.fsync(stream.fileno())
    os.chmod(source, 0o400)
    row = {'sha256': hashlib.sha256(body).hexdigest(), 'bytes': len(body)}
    fd = os.open(output, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        return clone_one(source, fd, row, clone)
    finally:
        os.close(fd)
