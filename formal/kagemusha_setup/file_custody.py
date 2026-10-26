"""Bounded diagnostic file custody; no parameter or proof construction API.

This owner charges output bytes/files/depth before writes and checks exact input
bytes under held nofollow directory descriptors. It does not claim crash
persistence, an OS allocation limit, or custody against malicious Python code.
"""
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat

from .custody import require

CHUNK = 1 << 20
MAX_READ_BYTES = 16 << 20


@dataclass(frozen=True)
class Limits:
    """Finite output budgets, with no historical circuit-sized reservation."""
    output_bytes: int = 1 << 20
    files: int = 64
    depth: int = 4

    def validate(self):
        require(type(self.output_bytes) is int and 1 <= self.output_bytes <= 32 << 20,
                'finite output byte budget')
        require(type(self.files) is int and 1 <= self.files <= 128 and
                type(self.depth) is int and 1 <= self.depth <= 8, 'finite output shape')
        return self


def identity(s): return s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_ctime_ns


def safe(path):
    path=Path(path)
    require(path.is_absolute() and '..' not in path.parts and len(path.parts)<=64,'absolute bounded path')
    for p in [*path.parents,path]:require(not p.is_symlink(),'no symbolic links')
    return path


def output_preflight(directory):
    """Reject unsafe or occupied names before opening any output file."""
    path=safe(Path(directory).absolute())
    require(not path.exists() and path.parent.is_dir(), 'fresh output with existing parent')
    return path


class Outputs:
    """One tree owner: charge every chunk before writing, then revalidate it."""
    def __init__(self,directory,limits):
        require(type(limits) is Limits, 'exact output limits')
        self.limits=limits.validate();self.root=output_preflight(directory)
        self.used=0;self.records={};self.active=set()
        directory=self.root
        directory.mkdir(mode=0o700)
        require(stat.S_IMODE(directory.stat().st_mode)==0o700,'owner-only output')
        self.root_id=(directory.stat().st_dev,directory.stat().st_ino)

    def check_root(self):
        """Require the original directory even when no files were emitted."""
        safe(self.root)
        try:
            current=self.root.lstat()
        except FileNotFoundError as error:
            raise ValueError('output root changed') from error
        require(stat.S_ISDIR(current.st_mode) and
                (current.st_dev,current.st_ino)==self.root_id, 'output root changed')

    def check_path(self,path):
        path=safe(Path(path));rel=path.relative_to(self.root)
        require(0<len(rel.parts)<=self.limits.depth,'output depth')
        self.check_root()
        return str(rel)

    @contextmanager
    def stream(self,path):
        name=self.check_path(path)
        require(name not in self.records and name not in self.active,'fresh output name')
        require(len(self.records)+len(self.active)<self.limits.files,
                'output file budget')
        Path(path).parent.mkdir(parents=True,exist_ok=True)
        self.active.add(name);digest=hashlib.sha256();count=0
        with Path(path).open('xb') as out:
            def write(raw):
                nonlocal count
                require(type(raw) is bytes and len(raw)<=CHUNK,'bounded output chunk')
                limit=self.limits.output_bytes
                require(len(raw)<=limit-self.used,'output byte budget before write')
                self.used+=len(raw);count+=len(raw);digest.update(raw)
                require(out.write(raw)==len(raw),'short output write')
            try:yield write
            finally:
                self.records[name]={'name':name,'bytes':count,'sha256':digest.hexdigest()}
                self.active.remove(name)

    def write(self,path,raw):
        require(type(raw) is bytes,'immutable output bytes')
        with self.stream(path) as emit:
            for i in range(0,len(raw),CHUNK):emit(raw[i:i+CHUNK])

    def save(self,path,value):
        with self.stream(path) as emit:
            for text in json.JSONEncoder(indent=2,sort_keys=True).iterencode(value):
                raw=text.encode()
                for i in range(0,len(raw),CHUNK):emit(raw[i:i+CHUNK])
            emit(b'\n')

    def verify(self):
        require(not self.active,'no partial live writer')
        self.check_root()
        actual=[]
        expected_dirs={str(p) for name in self.records for p in Path(name).parents if str(p)!='.'}
        def enumeration_failed(error):
            raise error
        for directory,subdirs,files in os.walk(self.root,followlinks=False,onerror=enumeration_failed):
            require(len(subdirs)+len(files)<=self.limits.files+16,'bounded output enumeration')
            for name in subdirs:
                child=Path(directory)/name;rel=self.check_path(child)
                require(not child.is_symlink() and rel in expected_dirs,'unexpected output directory')
            for name in files:
                path=Path(directory)/name;rel=self.check_path(path)
                require(rel in self.records,'unexpected output file')
                before=path.lstat();require(stat.S_ISREG(before.st_mode),'regular output')
                row=self.records[rel];require(before.st_size==row['bytes'],'retained output extent')
                digest=hashlib.sha256();count=0
                with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW),'rb') as stream:
                    opened=os.fstat(stream.fileno());require(identity(opened)==identity(before),'output changed before read')
                    while count<row['bytes']:
                        block=stream.read(min(CHUNK,row['bytes']-count));require(bool(block),'short output')
                        count+=len(block);digest.update(block)
                    require(not stream.read(1),'extended output')
                    require(identity(os.fstat(stream.fileno()))==identity(before),'output changed during read')
                require(identity(path.lstat())==identity(before) and digest.hexdigest()==row['sha256'],
                        'retained output hash/identity')
                actual.append(rel)
                require(len(actual)<=self.limits.files,'output file budget')
        self.check_root()
        require(set(actual)==set(self.records),'exact retained output namespace')
        return [self.records[name] for name in sorted(self.records)]


# Descriptor-relative file custody, independent of any circuit or original format.

@contextmanager
def guarded_file(path, extent, observation):
    """Hold nofollow parent descriptors; compare descriptor and namespace identities."""
    path = safe(path)
    require(len(path.parts) <= 64, 'path component bound')
    opened_dirs, stream = [], None
    try:
        current = Path(path.anchor)
        fd = os.open(str(current), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        opened_dirs.append((current, fd, os.fstat(fd)))
        for component in path.parts[1:-1]:
            current = current / component
            fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            opened_dirs.append((current, fd, os.fstat(fd)))
        before = os.stat(path.name, dir_fd=fd, follow_symlinks=False)
        require(stat.S_ISREG(before.st_mode) and before.st_size == extent, 'original extent/type')
        raw_fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        stream = os.fdopen(raw_fd, 'rb')
        opened = os.fstat(stream.fileno())
        require(identity(before) == identity(opened), 'original changed before read')
        observation['stat_identity_before'] = list(identity(before))
        yield stream
        finished = os.fstat(stream.fileno())
        selected = os.stat(path.name, dir_fd=fd, follow_symlinks=False)
        namespace = path.lstat()
        require(len({identity(s) for s in (before, opened, finished, selected, namespace)}) == 1,
                'original changed during read')
        for parent, parent_fd, prior in opened_dirs:
            now, named = os.fstat(parent_fd), parent.lstat()
            require(stat.S_ISDIR(named.st_mode) and (prior.st_dev, prior.st_ino)
                    == (now.st_dev, now.st_ino) == (named.st_dev, named.st_ino), 'original parent changed')
        observation['identities_unchanged'] = True
    finally:
        if stream is not None:
            stream.close()
        for _, fd, _ in reversed(opened_dirs):
            os.close(fd)


def read_original(path,pin,checkpoint=lambda:None,*,observation=None):
    """One full read/hash, retaining partial observations even on refusal."""
    extent,expected=pin
    observation={} if observation is None else observation
    observation.update(path=str(path),expected_bytes=extent,expected_sha256=expected,
                       bytes_read=0,bytes_hashed=0,extension_sentinel_bytes=None,passed=False)
    digest=hashlib.sha256();raw=bytearray()
    try:
        require(type(extent) is int and 0<extent<=MAX_READ_BYTES and type(expected) is str and len(expected)==64
                and all(c in '0123456789abcdef' for c in expected),
                'bounded original extent/digest')
        named=safe(path).lstat()
        observation.update(observed_namespace_bytes=named.st_size,namespace_identity=list(identity(named)))
        with guarded_file(path,extent,observation) as stream:
            while len(raw)<extent:
                block=stream.read(min(CHUNK,extent-len(raw)))
                observation['bytes_read']+=len(block)
                require(bool(block),'short original')
                raw.extend(block);digest.update(block);observation['bytes_hashed']+=len(block)
                checkpoint()
            sentinel=stream.read(1);observation['extension_sentinel_bytes']=len(sentinel)
            require(not sentinel,'extended original')
            require(digest.hexdigest()==expected,'exact original digest')
            result=bytes(raw);checkpoint()
        observation['passed']=True
        return result
    except BaseException as error:
        observation['error']=type(error).__name__+': '+str(error)[:1024]
        raise
    finally:observation['observed_sha256']=digest.hexdigest()
