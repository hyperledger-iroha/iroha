"""Retain the four direct validator children of a fixed scaling trial.

Requires Python 3.11+ and the pinned BLAKE3 package in scripts/requirements.txt.
This internal owner retains node configurations, block-store namespaces and
direct process handles. The caller retains and authenticates other generated
inputs, runtime dependencies, load/probe outputs and stopped-store
evidence. No phase transition is a release attestation. TODO: Wire this owner
into the fixed launcher after those admission and command boundaries are
implemented; remove the old external trial harness.
"""
from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path
import re
import stat
import subprocess
import time
import tomllib
from typing import Callable, NoReturn, TypeVar

import blake3

from resource_process import DarwinProcessReader, ExecutableImage, PinnedProcess, ProcessIdentity
from scaling_readiness import FourPeerReadiness, ReadyReceipt

_T = TypeVar('_T')
_MAX_STOP_NS = 300 * 1_000_000_000
_MAX_TRIAL_NS = 7200 * 1_000_000_000
# Matches iroha_config::base::toml::MAX_TOML_SOURCE_BYTES, used by iroha3d's
# INTEGRITY_BOUND_CONFIG_MAX_BYTES_V1 startup reader.
_MAX_CONFIG_BYTES = 1024 * 1024
_MAX_DIRECTORY_HANDLES = 128
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_FILE_FLAGS = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK


class LauncherError(ValueError):
    """A fixed public lifecycle code with no child output or secret material."""


def _require(condition: bool, code: str) -> None:
    if not condition:
        raise LauncherError(code)


def _path(value: Path) -> None:
    _require(type(value) is type(Path('/')) and value.anchor == '/'
             and str(value) == os.path.abspath(value)
             and 1 <= len(value.parts) - 1 <= 64
             and len(os.fsencode(value)) <= 4096, 'launch_path_invalid')


def _deadline(end_ns: int, maximum_ns: int) -> None:
    _require(type(end_ns) is int
             and 0 < end_ns - time.monotonic_ns() <= maximum_ns,
             'launch_deadline_invalid')


def _remaining(end_ns: int) -> float:
    remaining = end_ns - time.monotonic_ns()
    _require(remaining > 0, 'launch_deadline_exceeded')
    return remaining / 1_000_000_000


def _raise_public(error: BaseException) -> NoReturn:
    """Preserve cancellation control flow without carrying private exception text."""
    if isinstance(error, KeyboardInterrupt):
        raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit):
        raise SystemExit(1) from None
    if isinstance(error, GeneratorExit):
        raise GeneratorExit() from None
    raise LauncherError('launch_stage_failed') from None


@dataclass(frozen=True, slots=True)
class PeerLaunch:
    """Original configuration-role binding; no keys or serialized PID authority."""

    peer_id: str
    config: Path
    config_blake3: str
    block_store: Path

    def validate(self) -> None:
        """Admit bounded public arguments before any process creation."""
        _require(type(self.peer_id) is str
                 and re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', self.peer_id),
                 'launch_peer_invalid')
        _path(self.config)
        _path(self.block_store)
        _require(type(self.config_blake3) is str
                 and re.fullmatch(r'[0-9a-f]{64}', self.config_blake3),
                 'launch_config_digest_invalid')


def _directory_identity(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)


def _file_identity(info: os.stat_result) -> tuple[int, ...]:
    return (*_directory_identity(info), info.st_nlink, info.st_size,
            info.st_mtime_ns, info.st_ctime_ns)


class PeerLaunchInputs:
    """Retain original node configurations and mutable block-store namespaces.

    BLAKE3 is checked against the same original bytes passed to the daemon's
    ``--config-blake3`` boundary, under its one-MiB source limit. Configuration
    files remain immutable; store contents may change, but their original
    directory, ownership and every lexical parent edge must remain intact.
    Keep this context open through stopped-store proof replay and final census.
    Genesis, client configurations and loaded runtime dependencies still belong
    to the caller's independently retained input owner.
    """

    def __init__(self, peers: tuple[PeerLaunch, ...]):
        _require(not hasattr(self, '_peers'), 'launch_inputs_readmission')
        _require(type(peers) is tuple and len(peers) == 4
                 and all(type(peer) is PeerLaunch for peer in peers),
                 'launch_requires_four_roles')
        for peer in peers:
            peer.validate()
        _require(len({peer.config for peer in peers}) == 4
                 and len({peer.block_store for peer in peers}) == 4,
                 'launch_duplicate_role')
        self._peers = peers
        self._directories = {}
        self._files = []
        self._failed = False
        try:
            for peer in peers:
                parent = self._retain_directory(peer.config.parent)
                fd = os.open(peer.config.name, _FILE_FLAGS, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid()
                             and info.st_nlink == 1 and stat.S_IMODE(info.st_mode) == 0o600
                             and 0 < info.st_size <= _MAX_CONFIG_BYTES,
                             'launch_config_file_invalid')
                    identity = _file_identity(info)
                    digest, offset, contents = blake3.blake3(), 0, bytearray()
                    while offset < info.st_size:
                        raw = os.pread(fd, min(65536, info.st_size - offset), offset)
                        _require(bool(raw), 'launch_config_truncated')
                        digest.update(raw)
                        contents.extend(raw)
                        offset += len(raw)
                    _require(digest.hexdigest() == peer.config_blake3,
                             'launch_config_digest_mismatch')
                    # The exact bytes passed to iroha3d must actually select
                    # the retained store. An independently valid directory is
                    # not evidence for a different configured/default store.
                    config = tomllib.loads(contents.decode('utf-8'))
                    _require('extends' not in config and type(config.get('kura')) is dict
                             and type(config['kura'].get('store_dir')) is str
                             and config['kura']['store_dir'] == str(peer.block_store),
                             'launch_config_store_mismatch')
                    self._files.append((fd, parent, peer.config.name, identity))
                except BaseException:
                    os.close(fd)
                    raise
                store = self._retain_directory(peer.block_store)
                info = os.fstat(store)
                _require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700,
                         'launch_store_owner_invalid')
            self.validate(peers)
        except BaseException as error:
            self.close()
            _raise_public(error)

    def _retain_directory(self, path: Path) -> int:
        current, parent = Path('/'), None
        for index, name in enumerate(path.parts):
            current = Path('/') if index == 0 else current / name
            retained = self._directories.get(current)
            if retained is None:
                _require(len(self._directories) < _MAX_DIRECTORY_HANDLES,
                         'launch_directory_bound_exceeded')
                fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode), 'launch_ancestor_not_directory')
                    retained = (fd, parent, name, _directory_identity(info))
                    self._directories[current] = retained
                except BaseException:
                    os.close(fd)
                    raise
            parent = retained[0]
        return parent

    def validate(self, peers: tuple[PeerLaunch, ...]) -> None:
        """Reject retargeting or mutation without accepting a replacement owner."""
        try:
            _require(not self._failed and peers == self._peers
                     and len(self._files) == 4, 'launch_inputs_unavailable')
            self._validate_directories()
            for fd, parent, name, identity in self._files:
                _require(_file_identity(os.fstat(fd)) == identity
                         and _file_identity(os.stat(name, dir_fd=parent,
                                                   follow_symlinks=False)) == identity,
                         'launch_config_changed')
            self._validate_directories()
        except BaseException as error:
            self._failed = True
            _raise_public(error)

    def _validate_directories(self) -> None:
        for fd, parent, name, identity in self._directories.values():
            _require(_directory_identity(os.fstat(fd)) == identity
                     and _directory_identity(os.stat(name, dir_fd=parent,
                                                     follow_symlinks=False)) == identity,
                     'launch_directory_changed')

    def close(self) -> None:
        """Close only retained descriptors; never remove files or signal children."""
        self._failed = True
        while self._files:
            os.close(self._files.pop()[0])
        while self._directories:
            _, retained = self._directories.popitem()
            os.close(retained[0])

    def __enter__(self) -> PeerLaunchInputs:
        self.validate(self._peers)
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


class FourPeerRun:
    """Own original Popen handles and reuse the resource process identities.

    ``inputs`` retains the four exact configurations and block-store namespaces.
    ``verify_inputs`` rechecks the caller's other original inputs and runtime
    dependencies. Both owners must remain open through proof replay and final
    census. A callback cannot replace the retained node-input checks.
    """

    def __init__(self, peers: tuple[PeerLaunch, ...], image: ExecutableImage,
                 reader: DarwinProcessReader, inputs: PeerLaunchInputs,
                 verify_inputs: Callable[[], None],
                 trial_deadline_ns: int):
        _require(type(peers) is tuple and len(peers) == 4
                 and all(type(peer) is PeerLaunch for peer in peers),
                 'launch_requires_four_roles')
        for peer in peers:
            peer.validate()
        _require(len({peer.peer_id for peer in peers}) == 4
                 and len({peer.config for peer in peers}) == 4
                 and len({peer.block_store for peer in peers}) == 4,
                 'launch_duplicate_role')
        paths = tuple(peer.block_store for peer in peers)
        _require(all(left not in right.parents and right not in left.parents
                     for i, left in enumerate(paths) for right in paths[i + 1:]),
                 'launch_overlapping_stores')
        _require(isinstance(image, ExecutableImage) and callable(verify_inputs),
                 'launch_owner_invalid')
        _require(callable(getattr(reader, 'sample', None)), 'launch_reader_invalid')
        _require(all(path != store and store not in path.parents
                     for path in (image.path, *(peer.config for peer in peers))
                     for store in paths), 'launch_input_inside_mutable_store')
        _deadline(trial_deadline_ns, _MAX_TRIAL_NS)
        _require(type(inputs) is PeerLaunchInputs, 'launch_input_owner_invalid')
        inputs.validate(peers)
        self._peers, self._image, self._reader = peers, image, reader
        self._inputs = inputs
        self._image_binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
        self._verify_inputs = verify_inputs
        self._trial_deadline_ns = trial_deadline_ns
        self._children: list[subprocess.Popen] = []
        self._spawn_bindings: list[tuple[subprocess.Popen, int]] = []
        self._pinned: list[PinnedProcess] = []
        self._identities: list[ProcessIdentity] = []
        self._phase = 'admitted'
        self._stopped: set[int] = set()
        self._readiness: FourPeerReadiness | None = None

    def _verify(self) -> None:
        _require(self._phase == 'busy', 'launch_phase_invalid')
        _remaining(self._trial_deadline_ns)
        self._verify_image()
        self._inputs.validate(self._peers)
        _require(self._phase == 'busy', 'launch_phase_invalid')
        self._verify_inputs()
        _require(self._phase == 'busy', 'launch_phase_invalid')
        self._verify_image()
        self._inputs.validate(self._peers)
        _remaining(self._trial_deadline_ns)
        _require(self._phase == 'busy', 'launch_phase_invalid')

    def _verify_image(self) -> None:
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids)
                 == self._image_binding, 'launch_image_binding_changed')
        image.validate()

    def _child_handle(self, index: int) -> subprocess.Popen:
        """Check the original spawn binding without requiring a successful pin."""
        _require(len(self._children) == len(self._spawn_bindings),
                 'launch_child_binding_changed')
        child = self._children[index]
        handle, pid = self._spawn_bindings[index]
        _require(child is handle and type(pid) is int and 1 < pid <= (1 << 31) - 1
                 and child.pid == pid, 'launch_child_binding_changed')
        if index < len(self._pinned):
            _require(index < len(self._identities), 'launch_child_binding_changed')
            pinned, identity = self._pinned[index], self._identities[index]
            _require(pinned.pid == identity.pid == pid
                     and pinned.peer_id == self._peers[index].peer_id
                     and pinned.image is self._image and pinned.reader is self._reader
                     and pinned.identity == identity, 'launch_child_binding_changed')
        return child

    def _bound(self, index: int) -> tuple[subprocess.Popen, PinnedProcess]:
        _require(self._phase == 'busy', 'launch_phase_invalid')
        _remaining(self._trial_deadline_ns)
        self._verify_image()
        _require(self._phase == 'busy', 'launch_phase_invalid')
        _remaining(self._trial_deadline_ns)
        child, pinned = self._child_handle(index), self._pinned[index]
        identity = self._identities[index]
        _require(child.pid == pinned.pid == identity.pid
                 and pinned.peer_id == self._peers[index].peer_id
                 and pinned.image is self._image and pinned.reader is self._reader
                 and pinned.identity == identity, 'launch_child_binding_changed')
        return child, pinned

    def _live(self, indices: tuple[int, ...]) -> None:
        self._verify()
        for index in indices:
            child, pinned = self._bound(index)
            _require(child.poll() is None, 'launch_child_exited')
            observed = pinned.sample()
            self._bound(index)
            _require(observed.identity == self._identities[index],
                     'launch_child_binding_changed')
            _require(child.poll() is None, 'launch_child_exited')
        self._verify()

    def _stage(self, expected: str, completed: str,
               action: Callable[[], _T]) -> _T:
        try:
            _require(self._phase == expected, 'launch_phase_invalid')
            # A nested or caught failed transition must not be overwritten by
            # the outer operation returning normally.
            self._phase = 'busy'
            self._verify()
            _require(self._phase == 'busy', 'launch_phase_invalid')
            result = action()
            self._verify()
            _require(self._phase == 'busy', 'launch_phase_invalid')
            self._phase = completed
            return result
        except BaseException as error:
            self._phase = 'failed'
            # Child argv, parse failures and callback errors may contain private
            # input bytes. Retain only the fixed failure code in this interface.
            _raise_public(error)

    def launch_owned(self) -> tuple[PinnedProcess, ...]:
        """Start exactly four direct children, with no shell or inherited env."""
        def launch():
            for index, peer in enumerate(self._peers):
                self._live(tuple(range(index)))
                child = subprocess.Popen(
                    [str(self._image_binding[0]), '--config', str(peer.config),
                     '--config-blake3', peer.config_blake3],
                    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL, cwd='/', env={}, close_fds=True,
                    shell=False, start_new_session=False,
                )
                # Retain the direct child even if the first identity read fails.
                # Cleanup only uses this Popen, never a discovered or copied PID.
                self._children.append(child)
                self._spawn_bindings.append((child, child.pid))
                _require(type(child.pid) is int and 1 < child.pid <= (1 << 31) - 1
                         and len({item.pid for item in self._children}) == index + 1,
                         'launch_child_pid_invalid')
                self._pinned.append(PinnedProcess(peer.peer_id, child.pid,
                                                  self._image, self._reader))
                self._identities.append(self._pinned[-1].identity)
                self._live(tuple(range(index + 1)))
            return tuple(self._pinned)
        return self._stage('admitted', 'running', launch)

    def await_genesis_ready(self, step: FourPeerReadiness) -> tuple[ReadyReceipt, ...]:
        """Require four authenticated native receipts before admitting any load."""
        def ready():
            _require(type(step) is FourPeerReadiness
                     and step.trial_deadline_ns == self._trial_deadline_ns
                     and step.role_bindings == tuple((peer.peer_id, peer.config, peer.block_store)
                                                     for peer in self._peers),
                     'launch_readiness_binding_invalid')
            _require(self._readiness is None, 'launch_readiness_readmission')
            # Hold the owner before the first CLI spawn; failure retains cleanup
            # custody even when a command fails before its first image pin.
            self._readiness = step
            self._live((0, 1, 2, 3))
            result = step.collect(tuple(self._pinned), lambda: self._live((0, 1, 2, 3)))
            self._live((0, 1, 2, 3))
            _require(type(result) is tuple and len(result) == 4
                     and all(type(receipt) is ReadyReceipt and receipt.peer_id == peer.peer_id
                             and receipt.process == identity
                             for receipt, peer, identity in zip(result, self._peers, self._identities, strict=True)),
                     'launch_readiness_incomplete')
            return result
        return self._stage('running', 'ready', ready)

    def run_load(self, operation: Callable[[tuple[PinnedProcess, ...]], _T]) -> _T:
        """Bracket load/probe and peer3 local drain after authenticated readiness.

        The bounded operation must enforce load and local drain barriers. Its
        successful return alone is not authenticated transaction evidence.
        """
        def run():
            self._live((0, 1, 2, 3))
            result = operation(tuple(self._pinned))
            self._live((0, 1, 2, 3))
            return result
        return self._stage('ready', 'loaded', run)

    def _stop(self, index: int, end_ns: int) -> None:
        self._live((index,))
        _remaining(end_ns)
        child, _ = self._bound(index)
        child.terminate()
        # Popen.wait reaps this original child; a forced signal is never success.
        timeout = _remaining(end_ns)
        self._bound(index)
        status = child.wait(timeout=timeout)
        self._bound(index)
        _require(status == 0 and child.returncode == 0, 'launch_shutdown_unclean')
        _remaining(end_ns)
        self._stopped.add(index)

    def stop_peer3(self, deadline_ns: int) -> PeerLaunch:
        """Reap role3 cleanly while the three original proof peers remain live."""
        def stop():
            _deadline(deadline_ns, _MAX_STOP_NS)
            self._live((0, 1, 2, 3))
            self._stop(3, min(deadline_ns, self._trial_deadline_ns))
            self._live((0, 1, 2))
            return self._peers[3]
        return self._stage('loaded', 'peer3_stopped', stop)

    def collect_inputs(self, operation: Callable[[PeerLaunch, PinnedProcess], _T]) -> _T:
        """Observe peer3 tip and collect proofs with original peer0 still live."""
        def collect():
            _require(self._stopped == {3}, 'launch_stop_order_invalid')
            self._live((0, 1, 2))
            result = operation(self._peers[3], self._pinned[0])
            self._live((0, 1, 2))
            return result
        return self._stage('peer3_stopped', 'collected', collect)

    def stop_survivors(self, deadline_ns: int) -> None:
        """Reap remaining roles under one deadline, keeping peer0 until last."""
        def stop():
            _deadline(deadline_ns, _MAX_STOP_NS)
            end = min(deadline_ns, self._trial_deadline_ns)
            for index, live in ((2, (0, 1, 2)), (1, (0, 1)), (0, (0,))):
                self._live(live)
                self._stop(index, end)
            _require(self._stopped == {0, 1, 2, 3}, 'launch_reap_incomplete')
        self._stage('collected', 'stopped', stop)

    def verify_stopped(self) -> None:
        """Recheck retained inputs after later proof work, without another launch."""
        def verify():
            _require(len(self._children) == 4 and self._stopped == {0, 1, 2, 3}
                     and all(self._child_handle(index).returncode == 0 for index in range(4)),
                     'launch_reap_incomplete')
        self._stage('stopped', 'stopped', verify)

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Fail permanently and attempt bounded SIGTERM/reap of owned children.

        Return roles still awaiting reap. Never force-kill, restart, delete
        artifacts or signal a PID/process group. Even clean cleanup is failure.
        The caller must retain this owner and resolve any remaining children.
        """
        self._phase = 'failed'
        _deadline(deadline_ns, _MAX_STOP_NS)
        pending = []
        if self._readiness is not None:
            try:
                pending.extend(f'readiness-{role}' for role in self._readiness.cleanup(deadline_ns))
            except BaseException as error:
                if isinstance(error, Exception): pending.append('readiness')
                else: _raise_public(error)
        for index in reversed(range(len(self._children))):
            try:
                # Popen owns wait status and checks exit again when signalling.
                # A missing PinnedProcess during spawn rollback cannot cause
                # adoption of another process; only this handle is used here.
                child = self._child_handle(index)
                if child.poll() is None:
                    _remaining(deadline_ns)
                    self._child_handle(index)
                    child.terminate()
                timeout = _remaining(deadline_ns)
                self._child_handle(index)
                child.wait(timeout=timeout)
                self._child_handle(index)
            except BaseException as error:
                if isinstance(error, Exception):
                    pending.append(self._peers[index].peer_id)
                else:
                    _raise_public(error)
        return tuple(pending)
