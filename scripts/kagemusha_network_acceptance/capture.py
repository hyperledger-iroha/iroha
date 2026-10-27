"""Stable source observations for explicitly requested compiler captures."""
from pathlib import Path
import hashlib
import json
import os
import stat
import subprocess
import child_process
import source_audit
from paths import ROOT
REPO = ROOT

def digest(data):
    return hashlib.sha256(data).hexdigest()

def stable_read(path):
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise ValueError('input is not a regular file')
    with path.open('rb') as reader:
        opened = os.fstat(reader.fileno())
        data = reader.read()
        after = os.fstat(reader.fileno())
    final = path.lstat()
    signature = lambda value: (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
    if not signature(before) == signature(opened) == signature(after) == signature(final):
        raise ValueError('input changed during read')
    return data

def git(*arguments):
    return child_process.natural_run(
        ['git', *arguments], cwd=REPO, text=True, check=True, stdout=subprocess.PIPE
    ).stdout.strip()

def require_resolved_checkout():
    """Refuse pending merges, even with a clean index, before further work."""
    directory = REPO / '.git'
    if directory.is_symlink() or not directory.is_dir() or os.path.lexists(directory / 'commondir'):
        raise ValueError('original checkout required')
    if os.path.lexists(directory / 'MERGE_HEAD'):
        raise ValueError('merge in progress; conclude MERGE_HEAD before capture or execution')
    branch = git('branch', '--show-current')
    unmerged = git('diff', '--name-only', '--diff-filter=U').splitlines()
    if os.path.lexists(directory / 'MERGE_HEAD'):
        raise ValueError('merge in progress; conclude MERGE_HEAD before capture or execution')
    if branch != 'optimizations' or unmerged:
        raise ValueError('original resolved branch required at every stage')
    return {'branch': branch, 'unmerged': unmerged, 'merge_head': False}

def snapshot(package_roots=()):
    require_resolved_checkout()
    paths = child_process.natural_run(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'],
        cwd=REPO, check=True, stdout=subprocess.PIPE
    ).stdout.decode().split('\0')
    files, links, failures = {}, {}, {}
    for relative in sorted((set(paths) - {''}) | source_audit.package_paths(REPO, package_roots)):
        path = REPO / relative
        try:
            if path.is_symlink():
                links[relative] = os.readlink(path)
            elif path.is_file():
                files[relative] = digest(stable_read(path))
        except (OSError, ValueError) as error:
            failures[relative] = str(error)
    head = git('rev-parse', 'HEAD')
    return {
        'files': files, 'links': links, 'read_failures': failures,
        'head': head, **require_resolved_checkout(),
    }

def changes(before, after):
    result = {}
    for kind in ('files', 'links'):
        left, right = before[kind], after[kind]
        result[kind] = {
            name: [left.get(name), right.get(name)]
            for name in sorted(set(left) | set(right)) if left.get(name) != right.get(name)
        }
    result['metadata'] = {
        key: [before[key], after[key]] for key in ('head', 'branch', 'unmerged', 'merge_head')
        if before[key] != after[key]
    }
    return result
