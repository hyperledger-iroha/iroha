"""Physical custody of the five fixed outer-experiment public controls.

The outer owner supplies an original evidence-root descriptor, the complete
admitted budget, already authenticated immutable public projections and one
unchanged experiment guard. It owns native authority, runtime closure, the full
runs/resources census and semantic verdict. This module cannot issue PASS.
"""
from scaling_structural_identity import pin_experiment_budget
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import stat

from resource_bundle import ControlBinding, _directory_owner, _identity, _root_path
from resource_evidence_budget import (EvidenceBudget, canonical_run_budget_bytes,
    parse_run_budget, run_budget_inputs, select_run_budget)
from scaling_publication import (_close_owned, _file_identity, _flags, digest_file,
                                 names, publish_file)

_DIRECTORY = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_DIRECTORY
_STATIC = ('identity', 'plan', 'source_closure')
_ORDER = (*_STATIC, 'manifest', 'report')
_OUTER = frozenset(('runs', 'resources'))
_PATHS = {role: f'inputs/{role}.json' for role in _STATIC} | {
    'manifest': 'manifest.json', 'report': 'report.json'}


class ExperimentFileError(ValueError):
    """Closed physical experiment-custody failure without paths or contents."""


def _require(value):
    if not value: raise ExperimentFileError('experiment_file_custody_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise ExperimentFileError('experiment_file_custody_failed') from None


def _owned_budget(value):
    _require(type(value) is EvidenceBudget)
    # The selected pair supplies canonical framing only; all ten runs and every
    # experiment control are re-admitted and copied through the sole decoder.
    return parse_run_budget(run_budget_inputs(select_run_budget(value, 1, 'one_lane'))).experiment


def _budget_bytes(value):
    return canonical_run_budget_bytes(select_run_budget(value, 1, 'one_lane'))


@dataclass(slots=True)
class _File:
    fd: int
    identity: tuple
    flags: int
    binding: ControlBinding
    cap: int
    pin: tuple


class FixedExperimentFiles:
    """Retain three exact static inputs, then one manifest and one report.

    No caller can supply a path, alternate role, replacement guard or claimed
    success result. Public bytes are interpreted and authorized by the outer
    owner before publication here. The original construction guard remains
    mandatory through final verification under the original experiment scope.
    """

    def __init__(self, directory: Path, original_directory_fd: int,
                 allocation: EvidenceBudget, guard):
        self._chain=[];self._files={};self._inputs=None
        self._closed=False;self._failed=False;self._busy=True
        self._active=None;self._step=0;self._root_state=None;self._inputs_state=None
        try:
            _require(callable(guard))
            self._guard_callback=self._original_guard=guard
            self._allocation=_owned_budget(allocation)
            _require(tuple(item.label for item in self._allocation.static_files)==_STATIC
                     and all(item.size_bytes>0 for item in self._allocation.static_files)
                     and tuple(item.label for item in self._allocation.control_budgets)==('manifest','report'))
            self._caps={item.label:item.size_bytes for item in self._allocation.static_files}
            self._caps.update((item.label,item.max_bytes) for item in self._allocation.control_budgets)
            self._directory=Path(_root_path(directory));self._budget=_budget_bytes(self._allocation)
            self._budget_identity=pin_experiment_budget(self._allocation,self._budget)
            self._scope=(str(self._directory),tuple(self._caps.items()),self._budget)
            source_flags=_flags(original_directory_fd,readonly=True)
            source_info=os.fstat(original_directory_fd)
            _require(stat.S_ISDIR(source_info.st_mode) and source_info.st_uid==os.geteuid()
                     and stat.S_IMODE(source_info.st_mode)==0o700)
            self._source=(original_directory_fd,_directory_owner(source_info),source_flags)
            guard()
            parent=None
            for index,part in enumerate(self._directory.parts):
                name='/' if index==0 else part
                last=index==len(self._directory.parts)-1
                fd=os.dup(original_directory_fd) if last else os.open(name,_DIRECTORY,dir_fd=parent)
                created=None
                try:
                    info=os.fstat(fd);created=_identity(info)[:2]
                    _flags(fd,readonly=True)
                    _require(stat.S_ISDIR(info.st_mode))
                    owner=_directory_owner(info)
                    _require(_directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==owner)
                    if last:_require(owner==self._source[1])
                    self._chain.append((fd,parent,name,owner));parent=fd
                except BaseException:
                    if created is not None:_close_owned(fd,created)
                    raise
            self._root=self._chain[-1][0]
            self._guarded()
            os.mkdir('inputs',mode=0o700,dir_fd=self._root)
            fd=os.open('inputs',_DIRECTORY,dir_fd=self._root)
            created=None
            try:
                info=os.fstat(fd);created=_identity(info)[:2]
                _require(stat.S_ISDIR(info.st_mode) and info.st_uid==os.geteuid()
                         and stat.S_IMODE(info.st_mode)==0o700
                         and _identity(os.stat('inputs',dir_fd=self._root,follow_symlinks=False))==_identity(info)
                         and names(fd,0)==())
                self._inputs=(fd,_directory_owner(info));self._inputs_state=_identity(info)
                os.fsync(self._root)
            except BaseException:
                if created is not None:_close_owned(fd,created)
                raise
            self._guarded();self._busy=False
        except BaseException as error:
            self._failed=True;self.close();_failure(error)

    def _parent(self,role):
        return (self._inputs[0],f'{role}.json') if role in _STATIC else (self._root,_PATHS[role])

    def _base_check(self):
        _require(not self._closed and not self._failed
                 and self._guard_callback is self._original_guard
                 and (str(self._directory),tuple(self._caps.items()),self._budget_identity.checked_bytes(self._allocation,self._budget))==self._scope
                 and type(self._step) is int and 0<=self._step<=len(_ORDER)
                 and tuple(self._files)==_ORDER[:self._step]
                 and (self._active is None or self._step<len(_ORDER) and self._active==_ORDER[self._step]))
        fd,identity,flags=self._source
        _require(_flags(fd,readonly=True)==flags and _directory_owner(os.fstat(fd))==identity)
        for fd,parent,name,identity in self._chain:
            _flags(fd,readonly=True)
            _require(_directory_owner(os.fstat(fd))==identity
                     and _directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==identity)
        if self._inputs is not None:
            fd,identity=self._inputs;_flags(fd,readonly=True)
            _require(_directory_owner(os.fstat(fd))==identity
                     and _directory_owner(os.stat('inputs',dir_fd=self._root,follow_symlinks=False))==identity)
        for role,item in self._files.items():
            binding=item.binding
            _require((binding.label,binding.path,binding.sha256,item.cap)==item.pin)
            identity,flags=_file_identity(item.fd,item.cap,readonly=False)
            parent,name=self._parent(role)
            _require(identity==item.identity and flags==item.flags
                     and _identity(os.stat(name,dir_fd=parent,follow_symlinks=False))==identity)
        if self._inputs_state is not None and self._active not in _STATIC:
            _require(_identity(os.fstat(self._inputs[0]))==self._inputs_state)
        if self._root_state is not None and self._active not in ('manifest','report'):
            _require(_identity(os.fstat(self._root))==self._root_state)

    def _namespace(self):
        root_names=set(names(self._root,7))
        expected={'inputs'} if self._inputs is not None else set()
        expected.update(_PATHS[role] for role in ('manifest','report') if role in self._files)
        optional=set(_OUTER)
        if self._active in ('manifest','report'):
            optional.update((_PATHS[self._active],_PATHS[self._active]+'.publishing'))
        _require(expected<=root_names and root_names<=expected|optional)
        for name in root_names & _OUTER:
            info=os.stat(name,dir_fd=self._root,follow_symlinks=False)
            _require(stat.S_ISDIR(info.st_mode) and info.st_uid==os.geteuid()
                     and stat.S_IMODE(info.st_mode)==0o700)
        if self._inputs is not None:
            expected={f'{role}.json' for role in _STATIC if role in self._files}
            optional=set()
            if self._active in _STATIC:
                optional.update((f'{self._active}.json',f'{self._active}.json.publishing'))
            actual=set(names(self._inputs[0],5))
            _require(expected<=actual and actual<=expected|optional)
        if self._active is not None:
            parent,name=self._parent(self._active);found=[]
            for leaf in (name,name+'.publishing'):
                try:info=os.stat(leaf,dir_fd=parent,follow_symlinks=False)
                except FileNotFoundError:continue
                _require(stat.S_ISREG(info.st_mode) and info.st_uid==os.geteuid()
                         and stat.S_IMODE(info.st_mode)==0o600 and info.st_nlink in (1,2)
                         and 0<=info.st_size<=self._caps[self._active])
                found.append(_identity(info))
            if len(found)==2:_require(found[0]==found[1] and found[0][5]==2)

    def check_namespace(self):
        """Cheap original scope/file/metadata check: no callback or body hashing."""
        try:
            self._base_check();self._namespace();self._base_check()
        except BaseException as error:
            self._failed=True;_failure(error)

    def _guarded(self):
        self.check_namespace();self._guard_callback();self.check_namespace()

    def _enter(self):
        _require(not self._busy)
        self._busy=True;self._guarded()

    def _states(self):
        return (_identity(os.fstat(self._root)),_identity(os.fstat(self._inputs[0])))

    @property
    def directory(self):
        """Original public evidence root inside its still-retained scope."""
        self.check_namespace();return self._directory

    @property
    def allocation(self):
        """Return a separate canonical copy of the complete admitted budget."""
        self.check_namespace();value=_owned_budget(self._allocation);self.check_namespace()
        return value

    @property
    def controls(self):
        """Return originating physical bindings, without a semantic assertion."""
        self.check_namespace();return tuple(self._files[role].binding for role in _ORDER[:self._step])

    def _publish(self,role,raw):
        created=None
        try:
            self._enter()
            _require(self._step<len(_ORDER) and role==_ORDER[self._step]
                     and type(raw) is bytes and 0<len(raw)<=self._caps[role])
            if role in _STATIC:_require(len(raw)==self._caps[role])
            parent,name=self._parent(role);expected=frozenset(names(parent,7))
            previous=self._states();self._active=role
            digest=hashlib.sha256(raw).hexdigest()
            created=publish_file(parent,name,len(raw),self._caps[role],digest,
                lambda offset,count:raw[offset:offset+count],lambda:None,self._guarded,expected)
            binding=ControlBinding(role,_PATHS[role],digest)
            self._files[role]=_File(created.fd,created.identity,created.flags,binding,
                self._caps[role],(role,_PATHS[role],digest,self._caps[role]))
            created=None;self._step+=1;self._active=None
            if role in _STATIC:
                _require(_identity(os.fstat(self._root))==previous[0])
                self._inputs_state=_identity(os.fstat(self._inputs[0]))
            else:
                _require(_identity(os.fstat(self._inputs[0]))==previous[1])
                self._root_state=_identity(os.fstat(self._root))
            self._guarded();self._busy=False
            return binding
        except BaseException as error:
            self._failed=True
            if created is not None:_close_owned(created.fd,created.identity[:2])
            _failure(error)

    def publish_static(self,role:str,raw:bytes):
        """Publish the next exact-size authorized public static projection."""
        try:
            _require(type(role) is str and role in _STATIC)
            return self._publish(role,raw)
        except BaseException as error:self._failed=True;_failure(error)

    def publish_manifest(self,raw:bytes):
        """Publish the outer-derived manifest once, after all three inputs."""
        return self._publish('manifest',raw)

    def publish_report(self,raw:bytes):
        """Publish the outer-derived report once, after the original manifest."""
        return self._publish('report',raw)

    def verify(self):
        """Hash all currently published original controls under the same guard."""
        try:
            self._enter();before=self._states()
            for item in self._files.values():
                _require(digest_file(item.fd,item.identity,self._guarded)==item.binding.sha256)
            self._guarded();_require(self._states()==before)
            result=tuple(self._files[role].binding for role in _ORDER[:self._step])
            self._busy=False;return result
        except BaseException as error:self._failed=True;_failure(error)

    def read_control(self,binding:ControlBinding,*,max_bytes:int):
        """Read one exact original binding under an independently narrower cap."""
        try:
            self._enter()
            _require(type(binding) is ControlBinding and type(max_bytes) is int and max_bytes>=0)
            selected=tuple(item for item in self._files.values() if item.binding is binding)
            _require(len(selected)==1);item=selected[0]
            _require(item.identity[6]<=max_bytes<=item.cap)
            before=self._states();chunks=[];offset=0
            while offset<item.identity[6]:
                self._guarded();count=min(65536,item.identity[6]-offset)
                raw=os.pread(item.fd,count,offset)
                _require(type(raw) is bytes and 0<len(raw)<=count)
                chunks.append(raw);offset+=len(raw)
            raw=b''.join(chunks)
            _require(hashlib.sha256(raw).hexdigest()==binding.sha256)
            self._guarded();_require(self._states()==before)
            self._busy=False;return raw
        except BaseException as error:self._failed=True;_failure(error)

    def close(self):
        """Release only original owned FDs and preserve all public/diagnostic files."""
        if self._closed:return
        self._closed=True;self._failed=True
        owned=[(item.fd,item.identity[:2]) for item in self._files.values()]
        if self._inputs is not None:owned.append((self._inputs[0],self._inputs[1][:2]))
        owned.extend((fd,identity[:2]) for fd,_,_,identity in reversed(self._chain))
        for fd,identity in owned:_close_owned(fd,identity)
        self._files.clear();self._chain.clear();self._inputs=None

    def __enter__(self):return self

    def __exit__(self,*_):self.close()
