"""Retain the fixed JavaScript child's actual input bytes and physical owners.

Input-only eligibility, never a VerifiedNative/runtime/candidate receipt. The
separate parent must execute the genuine ABI checker with retained selected
Node/runtime/process inputs and authenticate source before publication. No
subprocess, install, native load or qualification report occurs in this module.
"""
from __future__ import annotations
from contextlib import AbstractContextManager
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import stat

import check_native_sdk_abi23_artifact as native
from sorafs_javascript_archive import ArchiveError, _require
from sorafs_javascript_child_tools import CHILD_TOOL_SHA256
from sorafs_javascript_dependencies import _json
from sorafs_javascript_input_files import HeldInputFile, absolute, cleanup_preserving, drain, record_cleanup
from sorafs_javascript_installed import InstalledProjection
from sorafs_javascript_installed_custody import OriginalInstalledTree
from sorafs_javascript_qualification_source import QualificationProjection
from sorafs_javascript_qualification_custody import OriginalQualificationTree
from sorafs_javascript_tree_custody import OriginalTree, TreeMember

MAX_INPUT_BYTES = 8 * 1024**2
MAX_TOOL_FILE_BYTES = 16 * 1024**2
MAX_TOOL_BYTES = 64 * 1024**2
MAX_NATIVE_BYTES = 1024**3
MAX_CHECKSUM_BYTES = 1024**2
_TARGET = re.compile(r'(darwin|linux)-(arm64|x64)-node24')


def _digest(value: str, *, commit: bool = False) -> None:
    _require(type(value) is str and re.fullmatch(r'[0-9a-f]{40}' if commit else r'[0-9a-f]{64}', value)
             and value != '0' * len(value), 'independently selected source identity differs')


def _row(path: str, raw: bytes, mode: int) -> dict:
    return {'path': path, 'sha256': hashlib.sha256(raw).hexdigest(), 'size': len(raw), 'mode': mode}


def _canonical_input(document: dict) -> bytes:
    """Bound encoded ASCII output incrementally before joining the final buffer."""
    output = bytearray()
    for piece in json.JSONEncoder(ensure_ascii=True, sort_keys=True, separators=(',', ':')).iterencode(document):
        raw = piece.encode('ascii')
        _require(len(output) + len(raw) + 1 <= MAX_INPUT_BYTES, 'child input canonical bytes exceed bound')
        output.extend(raw)
    output.append(10)
    return bytes(output)


def _overlaps(left: Path, right: Path) -> bool:
    return left == right or left in right.parents or right in left.parents


class OriginalJavascriptChildInput(AbstractContextManager):
    """Own one request descriptor and actual trees/files through future execution.

    Existing original projection validators remain the sole archive/source
    algorithms. The eight tool source pins are fixed reviewed code inputs.
    Expected candidate digests select the relation; they do not prove source
    approval, actual ABI execution, runtime closure or process completion.
    """
    def __init__(self, environment_root: Path, temporary_root: Path, input_path: Path, *,
                 installed: InstalledProjection, qualification: QualificationProjection,
                 tools_source_root: Path, native_path: Path, checksum_path: Path,
                 abi_manifest_path: Path, expected_source_commit: str,
                 expected_workspace_sha256: str, expected_native_source_sha256: str,
                 target: str):
        self._owners = []
        self._output = None
        self._closed = self._failed = self._checking = self._entered = False
        self._raw = None
        self._configuration = (environment_root, temporary_root, input_path, installed, qualification,
            tools_source_root, native_path, checksum_path, abi_manifest_path,
            expected_source_commit, expected_workspace_sha256, expected_native_source_sha256, target)

    def _add(self, owner):
        try:
            self._owners.append(owner)
        except BaseException as error:
            if not any(row is owner for row in self._owners):
                cleanup_preserving(error, owner)
            raise
        return owner

    def _tree(self, owner):
        self._add(owner)
        owner.__enter__()
        return owner

    def _file(self, path, maximum, **kwargs):
        return self._add(HeldInputFile(path, maximum, **kwargs))

    def __enter__(self):
        if self._checking:
            self._failed = True
        _require(not self._entered and not self._closed and not self._failed,
                 'child input construction is one-shot or refused')
        self._entered = self._checking = True
        try:
            self._prepare()
            _require(not self._closed and not self._failed, 'child input closed during construction')
        except BaseException as error:
            self._failed = True
            cleanup_preserving(error, self)
            raise
        finally:
            self._checking = False
        return self

    def _prepare(self) -> None:
        (environment, temporary, output, installed, qualification, source_root,
         native_path, checksum_path, manifest_path, commit, workspace, native_source, target) = self._configuration
        for path in (environment, temporary, output, source_root, native_path, checksum_path, manifest_path):
            absolute(path)
        _digest(commit, commit=True)
        _digest(workspace)
        _digest(native_source)
        _require(type(target) is str and _TARGET.fullmatch(target), 'child input requires a selected POSIX Node24 target')
        _require(output == environment / 'child-input.json'
                 and native_path.name == 'iroha_js_host.node'
                 and checksum_path == native_path.with_name('iroha_js_host.checksums.json'),
                 'child input output/native path selection differs')
        core, tools, modules = environment/'qualification/core', environment/'qualification/tools', environment/'node_modules'
        isolated = (core, tools, modules, temporary)
        _require(all(not _overlaps(left, right) for i,left in enumerate(isolated) for right in isolated[i+1:]),
                 'child input owned trees/temporary root overlap')
        _require(all(not _overlaps(path, owned) for path in (native_path,checksum_path,manifest_path,source_root,output)
                     for owned in isolated), 'child input originals/output overlap an executed tree or temporary root')
        _require(type(installed) is InstalledProjection and installed.environment_label == str(environment)
                 and type(qualification) is QualificationProjection,
                 'child input projections differ from their actual environment')
        # Hold original directory ancestors while allowing intended sibling work.
        self._environment = self._add(_PrivateDirectory(environment))
        self._temporary = self._add(_PrivateDirectory(temporary))
        self._tree(OriginalInstalledTree(modules, installed))
        self._tree(OriginalQualificationTree(core, qualification))
        tool_rows, tool_total = [], 0
        for name, expected in CHILD_TOOL_SHA256:
            original = self._file(source_root/name, MAX_TOOL_FILE_BYTES, mode=0o644)
            raw = original.raw
            tool_total += len(raw)
            _require(original.identity[0] == expected and tool_total <= MAX_TOOL_BYTES,
                     'child tool differs from the fixed reviewed original selection')
            tool_rows.append(TreeMember(name, raw, 0o644))
        self._tree(OriginalTree(tools, tuple(tool_rows)))
        artifact = self._file(native_path, MAX_NATIVE_BYTES, retain_bytes=False)
        checksum = self._file(checksum_path, MAX_CHECKSUM_BYTES)
        abi = self._file(manifest_path, native.MAX_MANIFEST_BYTES)
        _require(artifact.identity[1] > 0 and checksum.raw == installed.checksum_manifest,
                 'native original or installed checksum relation differs')
        manifest = native.validate_manifest(_json(abi.raw, native.MAX_MANIFEST_BYTES))
        _require(native.canonical_manifest_bytes(manifest) == abi.raw
                 and manifest['sdk'] == 'node' and manifest['target'] == target
                 and manifest['source_commit'] == commit
                 and manifest['workspace_source_manifest_sha256'] == workspace
                 and (manifest['artifact_sha256'], manifest['artifact_size']) == artifact.identity,
                 'original ABI manifest byte relation differs from selected inputs')
        # Only project source-context fields from the actual original checksum.
        # This is deliberately not a second checksum/signature-profile verifier:
        # the genuine installed loader owns raw/re-signed native verification.
        value = _json(checksum.raw, MAX_CHECKSUM_BYTES)
        platform = target.removesuffix('-node24')
        entry = value.get('entries', {}).get(platform) if type(value.get('entries')) is dict else None
        _require(type(entry) is dict and entry.get('source_git_revision') == commit
                 and entry.get('source_tree_clean') is True and entry.get('source_tree_sha256') == native_source,
                 'original checksum source context differs from independently selected inputs')
        document = {'schema': 'sorafs.javascript.child_input.v1', 'environmentRoot': str(environment),
            'temporaryRoot': str(temporary),
            'native': {'originalPath': str(native_path), 'sha256': artifact.identity[0], 'size': artifact.identity[1],
                'checksumSha256': checksum.identity[0], 'checksumSize': checksum.identity[1],
                'sourceCommit': commit, 'nativeSourceTreeSha256': native_source, 'workspaceSourceTreeSha256': workspace},
            'catalog': base64.b64encode(qualification.catalog).decode('ascii'),
            'source': [_row(row.path,row.content,0o644) for row in qualification.members],
            'installed': [_row(row.path,row.content,row.mode) for row in installed.members],
            'tools': [_row(row.path,row.content,row.mode) for row in tool_rows]}
        raw = _canonical_input(document)
        self._raw = raw
        self._output = self._file(output, MAX_INPUT_BYTES, payload=raw, mode=0o600)
        self._check()

    def _check(self) -> None:
        for owner in tuple(self._owners):
            owner.recheck()
        _require(not self._closed and not self._failed and self._output is not None
                 and self._output.raw == self._raw, 'child input owner changed during observation')

    def recheck(self) -> None:
        """Recheck every actual original and request fd; this is not a receipt."""
        if self._checking:
            self._failed = True
        _require(self._entered and not self._closed and not self._failed and not self._checking,
                 'child input owner is inactive, refused or reentrant')
        self._checking = True
        try:
            self._check()
        except BaseException:
            self._failed = True
            raise
        finally:
            self._checking = False

    @property
    def descriptor(self) -> int:
        """Borrow the original request descriptor for the parent's fixed fd3 map."""
        if self._checking:
            self._failed = True
        _require(self._entered and not self._closed and not self._failed and not self._checking
                 and self._output is not None, 'child input descriptor is not ready')
        return self._output.descriptor

    @property
    def sha256(self) -> str:
        """Return exact request-byte identity, without execution authority."""
        self.descriptor
        return self._output.identity[0]

    def close(self) -> None:
        """Detach and close all original owners once; never delete failed inputs."""
        self._closed = True
        if self._checking:
            self._failed = True
        owners, self._owners = tuple(self._owners), []
        self._output = None
        errors = []
        for owner in reversed(owners):
            try:
                owner.close()
            except BaseException as error:
                errors.append(error)
        if errors:
            failure = ArchiveError('child input owner cleanup failed')
            failure.cleanup_errors = tuple(errors)
            raise failure from errors[0]

    def __exit__(self, kind, value, traceback):
        if kind is not None:
            cleanup_preserving(value, self)
            return False
        try:
            self.recheck()
        except BaseException as error:
            cleanup_preserving(error, self)
            raise
        self.close()
        return False


class _PrivateDirectory:
    """Retain an original private root/ancestors without claiming its children."""
    def __init__(self, path):
        from release_manifest_signing import _open_release_output_parent
        self.path, self._parent = path, None
        self._failed = self._checking = False
        self._parent = _open_release_output_parent(path)
        try:
            self.recheck()
        except BaseException as error:
            cleanup_preserving(error, self)
            raise

    def recheck(self):
        if self._checking:
            self._failed = True
        _require(not self._failed and not self._checking and self._parent is not None,
                 'private input directory is refused or reentrant')
        self._checking = True
        try:
            self._observe()
            _require(not self._failed and self._parent is not None, 'private directory ended during observation')
        except BaseException:
            self._failed = True
            raise
        finally:
            self._checking = False

    def _observe(self):
        from release_manifest_signing import _open_release_output_parent
        _require(self._parent is not None, 'private input directory owner is closed')
        fd, expected, held = self._parent
        info = os.fstat(fd)
        _require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700,
                 'private input directory type/owner/mode differs')
        _fd, observed, descriptors = _open_release_output_parent(self.path)
        primary = None
        try:
            _require(self._parent is not None and observed == expected
                     and tuple((os.fstat(handle).st_dev,os.fstat(handle).st_ino) for handle in held) == expected,
                     'private input directory lost original ancestry')
            final = os.fstat(fd)
            _require((final.st_dev, final.st_ino, final.st_mode, final.st_uid, final.st_gid)
                     == (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid),
                     'private input directory policy changed during observation')
        except BaseException as error:
            primary = error
            raise
        finally:
            try:
                drain(descriptors)
            except BaseException as cleanup:
                if primary is None:
                    raise
                record_cleanup(primary, cleanup)

    def close(self):
        if self._checking:
            self._failed = True
        parent, self._parent = self._parent, None
        if parent is not None:
            drain(parent[2])
