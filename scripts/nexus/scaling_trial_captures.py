"""Bounded original resource-file custody around one trial's direct replay.

Resource replay owns JSON semantics. This owner reuses the physical bundle's
file reader and identity rules for the exact one-run flat namespace, retaining
only ancestor descriptors and aggregate census hashes across checks.
"""
from scaling_structural_identity import pin_run_budget
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import stat

from resource_bundle import (_MEMBER, _read_file, _identity, _directory_owner,
                             _DIRECTORY_FLAGS, _root_path)
from resource_evidence_budget import (PerRunResourceBudget, validate_run_budget,
    canonical_run_budget_bytes, CAPTURE_MANIFEST_BYTES)


class TrialCaptureError(ValueError):
    """Closed original-capture custody failure with no raw evidence contents."""


def require(value):
    if not value:
        raise TrialCaptureError('trial_capture_invalid')


@dataclass(frozen=True, slots=True)
class CaptureCensus:
    """Physical original files, independently bounded; no resource measurements."""
    files: int
    bytes: int
    census_sha256: str
    metadata_sha256: str


class TrialCaptures:
    """One admitted capture scope, with bounded descriptors and immutable census."""
    def __init__(self, directory: Path, allocation: PerRunResourceBudget, verify_inputs):
        self._chain=[];self._closed=False;self._failed=False;self._busy=False;self._transferred=False
        try:
            require(type(allocation) is PerRunResourceBudget and callable(verify_inputs))
            validate_run_budget(allocation)
            self._directory=Path(_root_path(directory));self._allocation=allocation
            self._budget=canonical_run_budget_bytes(allocation);self._guard=verify_inputs
            self._budget_identity=pin_run_budget(allocation,self._budget)
            self._retain('/',None)
            for part in self._directory.parts[1:]:self._retain(part,self._chain[-1][0])
            self._fd=self._chain[-1][0]
            info=os.fstat(self._fd)
            require(info.st_uid==os.geteuid() and stat.S_IMODE(info.st_mode)==0o700)
            self._root_state=_identity(info)
            self._census=None
            self._census=self._scan()
            self.check_namespace()
        except BaseException:
            self._failed=True;self.close();raise

    @property
    def directory(self):
        """Original capture path, never a new path-based admission."""
        try:
            self.check_namespace();value=self._directory;self.check_namespace()
            return value
        except Exception:
            self._failed=True
            raise TrialCaptureError('trial_capture_invalid') from None

    @property
    def allocation(self):
        """Original canonical admitted run allocation retained with this scope."""
        try:
            self.check_namespace();value=self._allocation;self.check_namespace()
            return value
        except Exception:
            self._failed=True
            raise TrialCaptureError('trial_capture_invalid') from None

    @property
    def census(self):
        self.check_namespace()
        require(self._census is not None)
        return self._census

    def _retain(self,name,parent):
        require(len(self._chain)<65)
        fd=os.open(name,_DIRECTORY_FLAGS,dir_fd=parent)
        try:
            info=os.fstat(fd)
            require(stat.S_ISDIR(info.st_mode))
            expected=_directory_owner(info)
            require(_directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==expected)
            self._chain.append((fd,parent,name,expected))
        except BaseException:os.close(fd);raise

    def check_namespace(self):
        """Cheap original namespace check suitable inside native command guards."""
        try:
            require(not self._closed and not self._failed)
            require(self._budget_identity.checked_bytes(self._allocation,self._budget)==self._budget)
            for fd,parent,name,expected in self._chain:
                require(_directory_owner(os.fstat(fd))==expected
                        and _directory_owner(os.stat(name,dir_fd=parent,follow_symlinks=False))==expected)
            require(_identity(os.fstat(self._fd))==self._root_state)
        except BaseException:self._failed=True;raise

    def _scan(self):
        require(not self._busy);self._busy=True
        try:
            self.check_namespace();self._guard();self.check_namespace()
            before=_identity(os.fstat(self._fd))
            allocation=self._allocation
            names=[]
            with os.scandir(self._fd) as entries:
                for entry in entries:
                    require(len(names)<allocation.member_count)
                    names.append(entry.name)
            names.sort();require(len(names)==allocation.member_count and len(set(names))==len(names))
            raw_census,metadata_census=hashlib.sha256(),hashlib.sha256()
            total=0;capture_bytes={}
            for name in names:
                self._guard();self.check_namespace()
                match=_MEMBER.fullmatch(name);require(match is not None)
                kind,number,peer,role=match.groups();sequence=int(number)
                require((kind=='preflight' and sequence==0)
                        or (kind=='sample' and 1<=sequence<=allocation.geometry.sample_count))
                cap=CAPTURE_MANIFEST_BYTES
                if peer is not None:
                    require(int(peer)<4)
                    cap=allocation.policy.status_body_bytes if role=='status' else allocation.policy.metrics_body_bytes
                metadata=os.stat(name,dir_fd=self._fd,follow_symlinks=False)
                identity=_identity(metadata)
                digest=_read_file(self._fd,name,metadata,cap,None,os.geteuid(),True)
                total+=metadata.st_size
                key=(kind,sequence);capture_bytes[key]=capture_bytes.get(key,0)+metadata.st_size
                require(total<=allocation.resource_bytes
                        and capture_bytes[key]<=allocation.experiment.bytes_per_capture)
                metadata_census.update(repr((name,identity)).encode()+b'\n')
                raw_census.update(repr((name,identity,digest)).encode()+b'\n')
                self._guard();self.check_namespace()
            # No long raw-body reads follow: detect early in-place changes during
            # later reads, including changes which leave the directory unchanged.
            self._guard();self.check_namespace()
            after_metadata=hashlib.sha256()
            for name in names:
                metadata=os.stat(name,dir_fd=self._fd,follow_symlinks=False)
                after_metadata.update(repr((name,_identity(metadata))).encode()+b'\n')
            require(after_metadata.digest()==metadata_census.digest())
            require(_identity(os.fstat(self._fd))==before==self._root_state)
            self.check_namespace()
            return CaptureCensus(len(names),total,raw_census.hexdigest(),metadata_census.hexdigest())
        except BaseException:self._failed=True;raise
        finally:self._busy=False

    def verify(self):
        """Re-read all original raw files under the initial physical admission."""
        try:
            require(self._census is not None)
            observed=self._scan()
            require(observed==self._census)
            return self._census
        except BaseException:self._failed=True;raise

    def check_metadata(self):
        """Fence the original complete census without callbacks or body reads.

        The experiment uses this after all runs' longer verifications so a later
        run cannot hide an earlier capture mutation. It retains the original
        admission; this is not a replacement scan which can accept new files.
        """
        try:
            self.check_namespace();require(self._census is not None)
            digest=hashlib.sha256()
            count=0
            # Admission proved every member of this exact finite grammar.
            # Generate its original lexical order without a new enumeration,
            # retained name array, callback or body read in the final fence.
            for sequence in range(self._allocation.geometry.sample_count+1):
                prefix=f'{"preflight" if sequence==0 else "sample"}-{sequence:010}'
                names=(f'{prefix}-peer-{peer:04}-{role}.body' for peer in range(4)
                       for role in ('metrics','status'))
                for name in (*names,prefix+'.json'):
                    info=os.stat(name,dir_fd=self._fd,follow_symlinks=False)
                    digest.update(repr((name,_identity(info))).encode()+b'\n');count+=1
            require(count==self._census.files==self._allocation.member_count
                    and digest.hexdigest()==self._census.metadata_sha256)
            self.check_namespace()
        except BaseException:self._failed=True;raise

    def transfer(self, verify_public):
        """Duplicate exact original custody once while the source guard still holds.

        The destination reuses the same bounded physical scanner with a distinct
        public-only guard. No path lookup admits a replacement directory and no
        caller-supplied census can create a transferred owner.
        """
        result=None
        try:
            require(type(self) is TrialCaptures and not self._transferred and callable(verify_public))
            self._transferred=True
            self.verify()
            result=object.__new__(TransferredCaptures)
            result._chain=[];result._closed=False;result._failed=False;result._busy=False;result._transferred=True
            result._directory=self._directory;result._allocation=self._allocation;result._budget=self._budget
            result._budget_identity=self._budget_identity
            result._root_state=self._root_state;result._census=self._census;result._guard=self._guard
            copied={}
            for fd,parent,name,expected in tuple(self._chain):
                original_parent=None if parent is None else copied[parent]
                duplicate=os.dup(fd)
                # Admission may observe a substituted source FD. Retain what
                # this call actually created for failure cleanup before asking
                # whether it is the expected original directory.
                created=_directory_owner(os.fstat(duplicate))
                result._chain.append((duplicate,original_parent,name,created))
                require(created==expected)
                copied[fd]=duplicate
            result._fd=copied[self._fd]
            result.verify();self.verify()
            result._guard=verify_public
            result.verify();self.verify()
            return result
        except BaseException:
            self._failed=True
            if result is not None:result.close()
            raise

    def close(self):
        """Close only retained ancestors, without deleting any evidence."""
        self._closed=True;pending=[]
        for row in reversed(self._chain):
            fd,_,_,expected=row
            try:
                require(_directory_owner(os.fstat(fd))==expected)
                os.close(fd)
            except (OSError,TrialCaptureError):pending.append(row)
        self._chain=list(reversed(pending))
        require(not pending)


class TransferredCaptures(TrialCaptures):
    """Original duplicated capture custody; obtainable only from a live source."""
    def __init__(self, *args, **kwargs):
        raise TrialCaptureError('trial_capture_invalid')

    def transfer(self, verify_public):
        """A transferred scope cannot mint further custody transfers."""
        self._failed=True
        raise TrialCaptureError('trial_capture_invalid')
