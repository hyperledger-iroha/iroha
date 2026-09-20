"""Original custody of the fixed five-file Python resource worker source pack.

The caller supplies an already prepared private directory and independently
admitted build/source pins. This owner reads files only. It neither discovers
imports nor proves interpreter/stdlib/dylib/shader closure or live resource
quotas. Keep it alive through every worker invocation and terminal child reap.
"""
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat

from resource_process import (_DIRECTORY_FLAGS, _FILE_FLAGS, _directory_identity,
                              _file_identity)

SOURCE_NAMES = ('resource_probe_worker.py', 'resource_probe.py', 'resource_process.py',
                'resource_evidence_budget.py', 'kura_resource_metrics.py')
MAX_SOURCE_BYTES = 1024 * 1024
MAX_TOTAL_SOURCE_BYTES = len(SOURCE_NAMES) * MAX_SOURCE_BYTES
MAX_ANCESTORS = 65
CHUNK_BYTES = 65536


class WorkerSourceError(ValueError):
    """Closed fixed-source custody failure, with no source text or private paths."""


def require(value):
    if not value:
        raise WorkerSourceError('worker_sources_invalid')


def _handle_identity(info):
    # Content/mode mutation invalidates admission but does not make this owned
    # descriptor foreign. A reused number for another inode must never close.
    return info.st_dev, info.st_ino, stat.S_IFMT(info.st_mode)


@dataclass(frozen=True, slots=True)
class WorkerSourcePin:
    """One independently expected original build-source identity and byte length."""
    name: str
    sha256: str
    bytes: int


class WorkerSourceFiles:
    """One exact five-source namespace, with bounded retained original handles."""
    def __init__(self, directory: Path, pins: tuple[WorkerSourcePin, ...]):
        self._handles=[];self._chain=[];self._files=[];self._closed=False;self._failed=False
        try:
            require(type(directory) is type(Path('/')) and directory.anchor=='/'
                    and str(directory)==os.path.abspath(directory)
                    and 1<=len(directory.parts)-1<=64 and len(os.fsencode(directory))<=4096)
            require(type(pins) is tuple and len(pins)==len(SOURCE_NAMES)
                    and all(type(pin) is WorkerSourcePin for pin in pins))
            self._pin_values=tuple((pin.name,pin.sha256,pin.bytes) for pin in pins)
            require(all(type(row[0]) is str for row in self._pin_values)
                    and tuple(row[0] for row in self._pin_values)==SOURCE_NAMES)
            for _,digest,length in self._pin_values:
                require(type(digest) is str and re.fullmatch('[0-9a-f]{64}',digest)
                        and type(length) is int and 0<length<=MAX_SOURCE_BYTES)
            require(sum(row[2] for row in self._pin_values)<=MAX_TOTAL_SOURCE_BYTES)
            self._directory,self._pins=directory,pins
            self._directory_value=str(directory)
            parent=None
            for name in directory.parts:
                require(len(self._chain)<MAX_ANCESTORS)
                fd,info=self._open(name,_DIRECTORY_FLAGS,parent)
                require(stat.S_ISDIR(info.st_mode))
                identity=_directory_identity(info)
                require(_directory_identity(os.stat(name,dir_fd=parent,follow_symlinks=False))==identity)
                self._chain.append((fd,parent,name,identity));parent=fd
            self._fd=parent;root=os.fstat(self._fd)
            require(root.st_uid==os.geteuid() and stat.S_IMODE(root.st_mode)==0o700)
            self._root_state=_file_identity(root)
            self._namespace()
            for name,digest,length in self._pin_values:
                self._namespace()
                fd,info=self._open(name,_FILE_FLAGS,self._fd)
                identity=_file_identity(info)
                require(stat.S_ISREG(info.st_mode) and info.st_uid==os.geteuid()
                        and info.st_nlink==1 and stat.S_IMODE(info.st_mode) in (0o400,0o600)
                        and info.st_size==length)
                self._files.append((fd,name,identity))
                raw_digest=hashlib.sha256();position=0
                while position<length:
                    chunk=os.pread(fd,min(CHUNK_BYTES,length-position),position)
                    require(bool(chunk) and len(chunk)<=length-position)
                    raw_digest.update(chunk);position+=len(chunk)
                require(raw_digest.hexdigest()==digest)
            # All long reads precede this full metadata/namespace closure; an
            # earlier file cannot be replaced or rewritten during a later read.
            self.validate()
        except BaseException as error:
            self._failed=True
            try:self.close()
            except WorkerSourceError:pass
            if isinstance(error,Exception):raise WorkerSourceError('worker_sources_invalid') from None
            raise

    def _open(self,name,flags,parent):
        fd=os.open(name,flags,dir_fd=parent)
        try:
            info=os.fstat(fd)
        except BaseException:
            os.close(fd)
            raise
        self._handles.append((fd,_handle_identity(info)))
        return fd,info

    def _namespace(self):
        require(not self._closed and not self._failed)
        require(type(self._directory) is type(Path('/'))
                and str(self._directory)==self._directory_value)
        require(type(self._pins) is tuple and len(self._pins)==len(SOURCE_NAMES)
                and all(type(pin) is WorkerSourcePin and type(pin.name) is str
                        and type(pin.sha256) is str and type(pin.bytes) is int
                        for pin in self._pins))
        require(tuple((pin.name,pin.sha256,pin.bytes) for pin in self._pins)==self._pin_values)
        for fd,parent,name,expected in self._chain:
            require(_directory_identity(os.fstat(fd))==expected
                    and _directory_identity(os.stat(name,dir_fd=parent,follow_symlinks=False))==expected)
        require(_file_identity(os.fstat(self._fd))==self._root_state)
        names=[]
        with os.scandir(self._fd) as entries:
            for entry in entries:
                require(len(names)<len(SOURCE_NAMES));names.append(entry.name)
        require(tuple(sorted(names))==tuple(sorted(SOURCE_NAMES)))
        require(_file_identity(os.fstat(self._fd))==self._root_state)

    def validate(self):
        """Check original files and the complete search directory; never refresh pins."""
        try:
            self._namespace();self._file_metadata()
            self._namespace()
            # Scandir is the final potentially deferred namespace operation.
            # Fence every retained leaf afterward, including initial admission.
            self._file_metadata()
        except Exception:
            self._failed=True
            raise WorkerSourceError('worker_sources_invalid') from None

    def _file_metadata(self):
        require(len(self._files)==len(SOURCE_NAMES))
        for fd,name,identity in self._files:
            require(_file_identity(os.fstat(fd))==identity
                    and _file_identity(os.stat(name,dir_fd=self._fd,follow_symlinks=False))==identity)

    @property
    def directory(self):
        """The original admitted search directory, without creating or resolving it."""
        self.validate();return self._directory

    @property
    def worker_path(self):
        """The sole supported worker entrypoint in this original source namespace."""
        self.validate();return self._directory/SOURCE_NAMES[0]

    @property
    def pins(self):
        """Exact original source pins; these alone are not runtime provenance."""
        self.validate();return self._pins

    def close(self):
        """Close only still-bound owned descriptors; preserve files and foreign handles."""
        self._closed=True;pending=[]
        for fd,identity in reversed(self._handles):
            try:
                require(identity is not None and _handle_identity(os.fstat(fd))==identity)
                os.close(fd)
            except (OSError,WorkerSourceError):pending.append((fd,identity))
        self._handles=list(reversed(pending))
        require(not pending)

    def __enter__(self):
        self.validate();return self

    def __exit__(self,*_):
        self.close()
