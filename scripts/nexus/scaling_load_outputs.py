"""Original private load inputs and receipt-bound journal/trace file custody.

Requires Python 3.11+. The CLI alone creates load outputs under already retained
parents. This owner never publishes measurements, decodes Norito, deletes partial
outputs, or closes another component's resources. Keep it through final replay.
"""
from dataclasses import dataclass
import fcntl
import hashlib
import os
from pathlib import Path
import stat

from resource_process import _directory_identity, _file_identity

DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
MAX_PRIVATE_BYTES = 1024 * 1024


class LoadOutputError(ValueError):
    """Closed public ownership failure, without private configuration bytes."""


def require(value):
    if not value: raise LoadOutputError('native_load_output_failed')


def path_check(path):
    require(type(path) is type(Path('/')) and path.anchor == '/' and str(path) == os.path.abspath(path)
            and '\x00' not in str(path) and 1 <= len(path.parts)-1 <= 64 and len(os.fsencode(path)) <= 4096)


@dataclass(frozen=True, slots=True)
class LoadPaths:
    """Declared fresh outputs and runtime-only worker configuration location."""
    resource_config: Path
    resource_capture_dir: Path
    transaction_trace: Path
    collector_journal: Path


@dataclass(frozen=True, slots=True)
class LoadArtifact:
    """One native-receipt-bound file under its original allocation and path."""
    label: str
    path: Path
    sha256: str
    bytes: int


class LoadFiles:
    """Retain original lexical parents, worker source/config and final outputs."""
    def __init__(self, paths, worker, worker_sha256, config_bytes, allocation):
        self.paths, self._directories, self._files, self._file_flags = paths, {}, {}, {}
        self._failed, self._complete = False, False
        self._allocation = allocation
        self._caps = (allocation.run.transaction_trace, allocation.journal)
        self._outputs = (paths.transaction_trace, paths.collector_journal)
        self._stage = paths.transaction_trace.with_name(paths.transaction_trace.name + '.collecting')
        self._worker,self._worker_sha=worker,worker_sha256
        self._snapshot = (tuple(getattr(paths,n) for n in paths.__dataclass_fields__),worker,worker_sha256,
                          tuple((cap.label,cap.max_bytes) for cap in self._caps))
        try:
            require(type(paths) is LoadPaths and type(config_bytes) is bytes and 0 < len(config_bytes) <= MAX_PRIVATE_BYTES)
            all_paths = (*tuple(getattr(paths, name) for name in paths.__dataclass_fields__), self._stage, worker)
            for path in all_paths: path_check(path)
            require(len(set(all_paths)) == len(all_paths))
            require(all(a not in b.parents and b not in a.parents for i,a in enumerate(all_paths) for b in all_paths[i+1:]))
            for path in all_paths: self._retain_directory(path.parent)
            for path in all_paths[:-1]:
                parent = os.fstat(self._directories[path.parent][0])
                require(parent.st_uid == os.geteuid() and stat.S_IMODE(parent.st_mode) == 0o700)
                self._absent(path)
            require(worker.name == 'resource_probe_worker.py')
            self._open_file(worker, worker_sha256, None, MAX_PRIVATE_BYTES, private=False)
            parent = self._directories[paths.resource_config.parent][0]
            fd = os.open(paths.resource_config.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=parent)
            try:
                offset = 0
                while offset < len(config_bytes):
                    count = os.write(fd, config_bytes[offset:]); require(count > 0); offset += count
                os.fsync(fd)
                before = _file_identity(os.fstat(fd))
                self._open_file(paths.resource_config, hashlib.sha256(config_bytes).hexdigest(), len(config_bytes), MAX_PRIVATE_BYTES)
                require(self._files[paths.resource_config][1] == before)
                os.fsync(parent)
            finally: os.close(fd)
            self.validate()
        except BaseException:
            self.close()
            raise

    def _retain_directory(self, path):
        current, parent = Path('/'), None
        for index,name in enumerate(path.parts):
            current = Path('/') if index == 0 else current/name
            if current not in self._directories:
                require(len(self._directories) < 128)
                fd = os.open(name, DIR_FLAGS, dir_fd=parent)
                try:
                    info = os.fstat(fd); require(stat.S_ISDIR(info.st_mode))
                    self._directories[current] = (fd,parent,name,_directory_identity(info))
                except BaseException: os.close(fd); raise
            parent = self._directories[current][0]
        return parent

    def _absent(self, path):
        try: os.stat(path.name,dir_fd=self._directories[path.parent][0],follow_symlinks=False)
        except FileNotFoundError: return
        require(False)

    def _open_file(self,path,digest,length,cap,private=True,guard=lambda: None):
        parent = self._directories[path.parent][0]
        fd = os.open(path.name,FILE_FLAGS,dir_fd=parent)
        try:
            info = os.fstat(fd); identity = _file_identity(info)
            require(stat.S_ISREG(info.st_mode) and info.st_uid==os.geteuid() and info.st_nlink==1
                    and stat.S_IMODE(info.st_mode) in ((0o600,) if private else (0o400,0o600,0o444,0o644))
                    and 0 < info.st_size <= cap and (length is None or info.st_size==length))
            total,hasher=0,hashlib.sha256()
            while total < info.st_size:
                guard(); raw=os.pread(fd,min(65536,info.st_size-total),total); require(bool(raw))
                hasher.update(raw); total+=len(raw)
            require(hasher.hexdigest()==digest and _file_identity(os.fstat(fd))==identity
                    and _file_identity(os.stat(path.name,dir_fd=parent,follow_symlinks=False))==identity)
            flags=(fcntl.fcntl(fd,fcntl.F_GETFL),fcntl.fcntl(fd,fcntl.F_GETFD))
            require(flags[0]&os.O_ACCMODE==os.O_RDONLY and flags[1]&fcntl.FD_CLOEXEC)
            self._files[path]=(fd,identity)
            self._file_flags[path]=flags
        except BaseException: os.close(fd); raise

    def _validate_directories(self):
        for fd,parent,name,identity in self._directories.values():
            require(_directory_identity(os.fstat(fd))==identity
                    and _directory_identity(os.stat(name,dir_fd=parent,follow_symlinks=False))==identity)
    def validate(self):
        require(not self._failed)
        require((tuple(getattr(self.paths,n) for n in self.paths.__dataclass_fields__),self._worker,self._worker_sha,
                 tuple((cap.label,cap.max_bytes) for cap in self._caps))==self._snapshot
                and self._outputs==(self.paths.transaction_trace,self.paths.collector_journal)
                and self._stage==self.paths.transaction_trace.with_name(self.paths.transaction_trace.name+'.collecting'))
        self._validate_directories()
        for path,(fd,identity) in self._files.items():
            require(_file_identity(os.fstat(fd))==identity
                    and _file_identity(os.stat(path.name,dir_fd=self._directories[path.parent][0],follow_symlinks=False))==identity
                    and (fcntl.fcntl(fd,fcntl.F_GETFL),fcntl.fcntl(fd,fcntl.F_GETFD))==self._file_flags[path])
        self._validate_directories()
        if self._complete: self._absent(self._stage)

    def before_launch(self):
        self.validate()
        for path in (*self._outputs,self._stage,self.paths.resource_capture_dir): self._absent(path)

    def capture(self, report, guard):
        require(not self._complete)
        self.validate()
        artifacts=[]
        for path,cap,prefix in zip(self._outputs,self._caps,('trace','collector_journal'),strict=True):
            digest,length=report[prefix+'_sha256'],report[prefix+'_bytes']
            self._open_file(path,digest,length,cap.max_bytes,guard=guard)
            artifacts.append(LoadArtifact(cap.label,path,digest,length))
        self._absent(self._stage)
        capture=self._retain_directory(self.paths.resource_capture_dir)
        info=os.fstat(capture)
        require(info.st_uid==os.geteuid() and stat.S_IMODE(info.st_mode)==0o700)
        self._complete=True
        self.validate();guard();self.validate()
        return tuple(artifacts)

    def public_descriptor(self, role: str) -> int:
        """Borrow only an original completed collector journal or trace FD.

        The recipient immediately duplicates the read-only FD and owns that
        duplicate. Source custody remains here until explicit close; private
        worker/config files and incomplete outputs cannot be exported.
        """
        try:
            require(type(role) is str and role in ('collector_journal','transaction_trace')
                    and self._complete)
            self.validate()
            path=getattr(self.paths,role)
            original=self._files[path]
            self.validate()
            fd,identity=original
            require(self._files[path] is original
                    and _file_identity(os.fstat(fd))==identity
                    and _file_identity(os.stat(path.name,dir_fd=self._directories[path.parent][0],follow_symlinks=False))==identity
                    and (fcntl.fcntl(fd,fcntl.F_GETFL),fcntl.fcntl(fd,fcntl.F_GETFD))==self._file_flags[path])
            return fd
        except BaseException as error:
            self._failed=True
            if isinstance(error,KeyboardInterrupt):raise KeyboardInterrupt() from None
            if isinstance(error,SystemExit):raise SystemExit(1) from None
            if isinstance(error,GeneratorExit):raise GeneratorExit() from None
            raise LoadOutputError('native_load_output_failed') from None

    def close(self):
        self._failed=True; failures=[]
        def close_original(fd,identity,flags=None):
            try:
                info=os.fstat(fd)
                require((info.st_dev,info.st_ino)==identity[:2]
                        and (flags is None or (fcntl.fcntl(fd,fcntl.F_GETFL),fcntl.fcntl(fd,fcntl.F_GETFD))==flags))
                os.close(fd)
            except (OSError,LoadOutputError) as error:failures.append(error)
        while self._files:
            path,(fd,identity)=self._files.popitem()
            flags=self._file_flags.pop(path,None)
            close_original(fd,identity,flags)
        while self._directories:
            fd,_,_,identity=self._directories.popitem()[1]
            close_original(fd,identity)
        require(not failures)
