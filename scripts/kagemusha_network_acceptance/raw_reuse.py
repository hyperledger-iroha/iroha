"""Optional pinned inactive DATA intake; current Rust import remains authority."""
from pathlib import Path
import os
import stat

from paths import QUAL
from raw_reuse_policy import bounded, doc, pin, require, sha

SCHEMA = 'kagemusha.native-finality.raw-original-reuse.v1'
MAX_FILES = 4096 * 3
MAX_FILE_BYTES = 1 << 30
MAX_POOL_BYTES = 128 << 30
IDENTITY_FIELDS = {'device', 'inode', 'mode', 'bytes', 'mtime_ns', 'ctime_ns', 'nlink'}


def identity(info):
    return {'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode,
            'bytes': info.st_size, 'mtime_ns': info.st_mtime_ns,
            'ctime_ns': info.st_ctime_ns, 'nlink': info.st_nlink}


def directory(path):
    require(path.is_absolute() and path.resolve(strict=True) == path,
            'unaliased absolute raw directory required')
    info = path.lstat()
    require(stat.S_ISDIR(info.st_mode), 'ordinary raw directory required')
    return {'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode}


def admit(selected):
    """Validate the bounded inventory and exact pre-run metadata, without hashing 65 GiB."""
    require(type(selected) is dict and set(selected) == {'path', 'sha256'},
            'raw reuse requires explicit inventory path and SHA')
    path = Path(selected['path'])
    require(path.is_absolute() and path.is_relative_to(QUAL), 'retained raw inventory required')
    value = doc(path, pin(selected['sha256']))
    require(type(value) is dict and set(value) == {'schema', 'root', 'directory_identity', 'files', 'scope'},
            'exact raw inventory fields required')
    require(value['schema'] == SCHEMA and type(value['scope']) is str, 'raw inventory schema')
    root = Path(value['root'])
    require(root.is_relative_to(QUAL) and root.name == 'originals', 'retained originals directory required')
    expected_dir = value['directory_identity']
    require(type(expected_dir) is dict and set(expected_dir) == {'device', 'inode', 'mode'}
            and all(type(v) is int for v in expected_dir.values()), 'raw directory identity fields')
    files = value['files']
    require(type(files) is dict and 0 < len(files) <= MAX_FILES, 'bounded nonempty raw namespace required')
    total = 0
    for name, row in files.items():
        require(pin(name) == name and type(row) is dict and set(row) == {'sha256', 'identity'}
                and row['sha256'] == name, 'exact content-addressed raw row required')
        ident = row['identity']
        require(type(ident) is dict and set(ident) == IDENTITY_FIELDS
                and all(type(v) is int for v in ident.values()), 'exact raw identity fields')
        require(stat.S_ISREG(ident['mode']) and 0 < ident['bytes'] <= MAX_FILE_BYTES
                and ident['nlink'] >= 1, 'bounded regular raw identity required')
        total += ident['bytes']
    require(total <= MAX_POOL_BYTES, 'raw pool exceeds unchanged logical output cap')
    check(value, hash_bytes=False)
    return value


def check(value, *, hash_bytes, output_originals=None):
    """Pre-run exact custody; post-run permits only the subject's one retained hard link."""
    root = Path(value['root'])
    require(directory(root) == value['directory_identity'], 'raw directory identity changed')
    if output_originals is not None:
        require(output_originals.is_absolute() and output_originals.resolve() == output_originals
                and not output_originals.is_relative_to(root) and not root.is_relative_to(output_originals),
                'raw/output directories overlap or alias')
        if output_originals.exists():
            directory(output_originals)
    expected = value['files']
    require({entry.name for entry in root.iterdir()} == set(expected), 'raw namespace changed')
    linked = 0
    total = 0
    observed = {}
    for name, row in sorted(expected.items()):
        path = root / name
        require(path.resolve(strict=True) == path, 'raw file alias refused')
        before = identity(path.lstat())
        require(stat.S_ISREG(before['mode']), 'raw file type changed')
        old = row['identity']
        target = None if output_originals is None else output_originals / name
        if before != old and target is not None:
            require(target.exists(), 'changed raw identity lacks its retained subject link')
            require(target.resolve(strict=True) == target, 'raw output alias refused')
            target_info = identity(target.lstat())
            require(stat.S_ISREG(target_info['mode'])
                    and (target_info['device'], target_info['inode']) == (before['device'], before['inode']),
                    'raw output is not the selected original hard link')
            require(all(before[key] == old[key] for key in IDENTITY_FIELDS - {'ctime_ns', 'nlink'})
                    and before['nlink'] == old['nlink'] + 1, 'unexpected raw hard-link mutation')
            linked += 1
        else:
            require(before == old, 'raw identity changed')
        if hash_bytes:
            require(sha(path) == row['sha256'], 'raw content-address hash differs')
        require(identity(path.lstat()) == before, 'raw original changed during custody inspection')
        total += before['bytes']
        observed[name] = before
    require(all(identity((root / name).lstat()) == ident for name, ident in observed.items()),
            'raw metadata changed across custody pass')
    require(directory(root) == value['directory_identity']
            and {entry.name for entry in root.iterdir()} == set(expected), 'raw namespace changed during custody inspection')
    return {'root': str(root), 'files': len(expected), 'logical_bytes': total,
            'content_hashes_verified': hash_bytes, 'controlled_subject_hard_links': linked,
            'authority': 'untrusted raw DATA; current Rust descriptor/VK/PK import required'}


def transport(value, out):
    """Emit only the production intake's roots JSON, never historical signed grants."""
    from raw_reuse_policy import save
    root = Path(value['root'])
    require(not out.is_relative_to(root) and not root.is_relative_to(out), 'raw/output overlap')
    require(root.stat().st_dev == out.stat().st_dev, 'raw/output filesystems differ')
    return save(out / 'reuse-inputs.json', [str(root)])


def published_transport(out):
    require(bounded(out / 'reuse-inputs.json', 16_384)
            == bounded(out / 'catalog/reuse-inputs.json', 16_384), 'native reuse transport differs or is missing')
