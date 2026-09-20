"""Invoke one fixed native four-validator scaling generator under original custody.

Requires Python 3.11+, a retained Kagami executable/kernel reader, and an explicit
runtime dependency verifier. The development seed is supplied only in memory.
No shell, environment switch, configurable command or Python Norito writer is
used. Keep GeneratedInputs open through daemon execution and final proof checks.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import ipaddress
import os
from pathlib import Path
import re
import stat
import time
from typing import Callable
from urllib.parse import urlsplit

from resource_process import ExecutableImage, ProcessIdentity, _directory_identity, _file_identity
from scaling_command import BoundedCommand, MAX_TRIAL_NS
from scaling_seed_pipe import DevelopmentSeedPipe
from scaling_readiness_inputs import (ReadinessInputs, ReadinessRole, GeneratedAccount,
    GeneratedArtifact, MAX_ANCHORS_BYTES)

_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
_FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
_RUNTIME_PATHS = (
    ('kura.store_dir', 'kura'),
    ('snapshot.store_dir', 'state/snapshot'),
    ('soracloud_runtime.state_dir', 'state/soracloud_runtime'),
    ('tiered_state.cold_store_root', 'state/tiered_state'),
    ('tiered_state.da_store_root', 'state/da_wsv_snapshots'),
    ('streaming.session_store_dir', 'state/streaming'),
    ('network.soranet_handshake.pow.revocation_store_path', 'state/soranet/ticket_revocations.norito'),
    ('torii.data_dir', 'state/torii'),
    ('torii.da_ingest.replay_cache_store_dir', 'state/torii/da_replay'),
    ('torii.da_ingest.manifest_store_dir', 'state/torii/da_manifests'),
    ('sorafs.discovery.replay_checkpoint_path', 'state/torii/sorafs_discovery_replay.nrt'),
    ('sorafs.storage.data_dir', 'state/sorafs'),
    ('sorafs.por.state_dir', 'state/sorafs/por'),
)


class GeneratorError(ValueError):
    """A closed public failure code that cannot contain development secrets."""


def _require(value):
    if not value: raise GeneratorError('fixed_generator_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise GeneratorError('fixed_generator_failed') from None


def _path(value):
    _require(type(value) is type(Path('/')) and value.anchor == '/'
             and str(value) == os.path.abspath(value) and 1 <= len(value.parts) - 1 <= 64
             and len(os.fsencode(value)) <= 4096)


def _host(value):
    _require(type(value) is str and value.isascii() and 0 < len(value) <= 253
             and '%' not in value)
    try:
        return str(ipaddress.ip_address(value))
    except ValueError:
        _require(':' not in value and all(re.fullmatch(r'[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?', part)
                 for part in value.split('.')) and not all(part.isdigit() for part in value.split('.')))
        return value


def _endpoint(value, host, port, literal):
    _require(type(value) is str and 0 < len(value) <= 2048 and all(32 <= ord(ch) < 127 for ch in value))
    if literal:
        # Native config parsing owns the CRC and canonical address spelling.
        # This projection compares its retained host/port; it writes no Norito.
        _require(value.startswith('addr:') and re.fullmatch(r'[0-9A-F]{4}', value[-4:])
                 and value[-5:-4] == '#')
        body = value[5:-5]
        parsed = urlsplit('tcp://' + body)
        _require(not parsed.path)
    else:
        parsed = urlsplit(value)
        _require(parsed.scheme == 'http' and parsed.path == '/')
    _require(parsed.hostname is not None and _host(parsed.hostname) == _host(host)
             and parsed.port == port and parsed.username is None and parsed.password is None
             and not parsed.query and not parsed.fragment)


@dataclass(frozen=True, slots=True)
class GeneratorPlan:
    """Public fixed generation selection; no development seed is retained here."""
    chain_id: str
    lane_count: int
    account_count: int
    bind_host: str
    public_host: str
    base_api_port: int
    base_p2p_port: int

    def validate(self):
        """Validate the complete fixed four-peer geometry before any filesystem change."""
        _require(type(self.chain_id) is str and re.fullmatch(r'[A-Za-z0-9._:-]{1,128}', self.chain_id))
        _require(type(self.lane_count) is int and self.lane_count in (1, 4)
                 and type(self.account_count) is int and 4 <= self.account_count <= 64
                 and self.account_count % 4 == 0)
        _require(_host(self.bind_host) == self.bind_host and _host(self.public_host) == self.public_host)
        _require(all(type(port) is int and 1 <= port <= 65532 for port in (self.base_api_port, self.base_p2p_port)))
        _require(set(range(self.base_api_port, self.base_api_port + 4)).isdisjoint(
            range(self.base_p2p_port, self.base_p2p_port + 4)))


@dataclass(frozen=True, slots=True)
class GeneratedPeer:
    """One original public role plus retained native transport address literals."""
    role: ReadinessRole
    p2p_bind: str
    p2p_public: str
    torii_bind: str
    state_root: Path
    runtime_paths: tuple[tuple[str, str], ...]


@dataclass(frozen=True, slots=True)
class GenerationReceipt:
    """Public generation evidence, without private seed, config or output text."""
    plan: GeneratorPlan
    generator_sha256: str
    process: ProcessIdentity
    anchors_sha256: str
    anchors_bytes: int
    genesis_hash: str
    context_id: str
    network_id: str
    genesis_public_key: str
    chain_discriminant: int
    peers: tuple[GeneratedPeer, ...]
    accounts: tuple[GeneratedAccount, ...]
    artifacts: tuple[GeneratedArtifact, ...]


class _FreshNamespace:
    """Original new root and named ancestor edges; never adopts an existing root."""

    def __init__(self, root):
        _path(root)
        self.root, self.entries = root, {}
        try:
            parent = self.retain(root.parent)
            os.mkdir(root.name, 0o700, dir_fd=parent)
            self.retain(root)
            self.empty(root)
            self.validate()
        except BaseException:
            self.close()
            raise

    def retain(self, path):
        current, parent = Path('/'), None
        for index, name in enumerate(path.parts):
            current = Path('/') if index == 0 else current / name
            item = self.entries.get(current)
            if item is None:
                _require(len(self.entries) < 128)
                fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode))
                    if current == self.root or self.root in current.parents:
                        _require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
                    item = (fd, parent, name, _directory_identity(info))
                    self.entries[current] = item
                except BaseException:
                    os.close(fd)
                    raise
            parent = item[0]
        return parent

    def validate(self):
        _require(bool(self.entries))
        for fd, parent, name, identity in self.entries.values():
            _require(_directory_identity(os.fstat(fd)) == identity
                     and _directory_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)

    def census(self, path, expected):
        self.validate()
        found = set()
        with os.scandir(self.entries[path][0]) as entries:
            for entry in entries:
                _require(len(found) < len(expected) and entry.name in expected
                         and entry.is_dir(follow_symlinks=False))
                found.add(entry.name)
        _require(found == expected)
        self.validate()

    def empty(self, path):
        self.census(path, set())

    def admit_stores(self):
        """Retain the exact initial four role namespaces; all leaves start empty."""
        storage = self.root / 'storage'
        paths = [storage]
        for index in range(4):
            peer = storage / f'peer{index}'
            paths.extend((peer, peer / 'kura', peer / 'state'))
        for path in paths: self.retain(path)
        # Directory identity deliberately allows live writes. Initial admission
        # additionally brackets every census with full metadata for all roles.
        snapshots = tuple((path, _file_identity(os.fstat(self.entries[path][0]))) for path in paths)
        self.census(storage, {f'peer{i}' for i in range(4)})
        for index in range(4):
            peer = storage / f'peer{index}'
            self.census(peer, {'kura', 'state'})
            for name in ('kura', 'state'):
                self.empty(peer / name)
        self.validate_initial_stores(snapshots)
        return snapshots

    def validate_initial_stores(self, snapshots):
        """Reject any initial namespace change before public input admission."""
        _require(len(snapshots) == 13)
        self.validate()
        for path, snapshot in snapshots:
            fd, parent, name, _ = self.entries[path]
            _require(_file_identity(os.fstat(fd)) == snapshot
                     and _file_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == snapshot)

    def close(self):
        failures = []
        while self.entries:
            fd = self.entries.popitem()[1][0]
            try: os.close(fd)
            except OSError as error: failures.append(error)
        _require(not failures)


class GeneratedInputs:
    """Retained originals; this owner must outlive readiness, load and proof replay."""
    __slots__ = ('_owner', '_inputs', '_receipt')

    def __init__(self, owner, inputs, receipt):
        self._owner, self._inputs, self._receipt = owner, inputs, receipt

    @property
    def inputs(self):
        self.validate()
        return self._inputs

    @property
    def receipt(self):
        self.validate()
        return self._receipt

    def validate(self):
        """Recheck generation identity and every original namespace/file edge."""
        self._owner.validate()
        _require(self._owner._generated is self and self._owner._inputs is self._inputs)

    def close(self):
        """Close generation-owned descriptors after all consuming lifetimes end."""
        self._owner.close()

    def __enter__(self):
        self.validate()
        return self

    def __exit__(self, *_): self.close()


class FixedGenerator:
    """One native generation attempt with the selected original absolute deadline.

    verify_runtime authenticates the caller-owned original native runtime closure.
    GeneratedInputs proves fresh generation and retained originals; the daemon
    caller must additionally own every referenced runtime artifact/path through
    its full lifetime. Initial empty directories do not qualify that runtime.
    """

    def __init__(self, plan: GeneratorPlan, output_directory: Path, image: ExecutableImage,
                 reader, trial_deadline_ns: int, verify_runtime: Callable[[], None]):
        try:
            _require(not hasattr(self, '_plan') and type(plan) is GeneratorPlan)
            plan.validate()
            _path(output_directory)
            _require(isinstance(image, ExecutableImage) and callable(getattr(reader, 'sample', None))
                     and callable(verify_runtime) and type(trial_deadline_ns) is int
                     and 0 < trial_deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS)
            _require(image.path != output_directory and output_directory not in image.path.parents)
            image.validate()
            self._plan, self._snapshot = plan, tuple(getattr(plan, field) for field in plan.__dataclass_fields__)
            self._end, self._verify_runtime = trial_deadline_ns, verify_runtime
            self._image, self._image_binding = image, (image.path, image.fd, image.identity, image.sha256, image.uuids)
            self._phase, self._anchor, self._inputs, self._generated = 'admitted', None, None, None
            self._seed_pipe = None
            self._namespace = _FreshNamespace(output_directory)
            self._commands = BoundedCommand(image, reader, trial_deadline_ns, self._verify)
        except BaseException as error:
            if hasattr(self, '_namespace'): self._namespace.close()
            _failure(error)

    @property
    def trial_deadline_ns(self): return self._end

    def _verify(self):
        _require(self._phase in ('generating', 'generated') and time.monotonic_ns() < self._end)
        _require(tuple(getattr(self._plan, field) for field in self._plan.__dataclass_fields__) == self._snapshot
                 and self._commands.deadline_ns == self._end)
        self._namespace.validate()
        self._verify_image_anchor()
        self._verify_runtime()
        _require(self._phase in ('generating', 'generated') and time.monotonic_ns() < self._end)
        _require(tuple(getattr(self._plan, field) for field in self._plan.__dataclass_fields__) == self._snapshot
                 and self._commands.deadline_ns == self._end)
        self._namespace.validate()
        self._verify_image_anchor()
        if self._inputs is not None: self._inputs.validate()

    def _verify_image_anchor(self):
        if self._seed_pipe is not None:
            self._seed_pipe.validate()
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._image_binding)
        image.validate()
        if self._anchor is not None:
            fd, identity = self._anchor
            root = self._namespace.entries[self._namespace.root][0]
            _require(_file_identity(os.fstat(fd)) == identity
                     and _file_identity(os.stat('genesis-anchors.json', dir_fd=root, follow_symlinks=False)) == identity)

    def _capture_anchor(self, raw):
        _require(type(raw) is bytes and 0 < len(raw) <= MAX_ANCHORS_BYTES)
        root = self._namespace.entries[self._namespace.root][0]
        fd = os.open('genesis-anchors.json', _FILE_FLAGS, dir_fd=root)
        self._anchor = (fd, ())
        info = os.fstat(fd)
        self._anchor = (fd, _file_identity(info))
        _require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_nlink == 1
                 and stat.S_IMODE(info.st_mode) == 0o600 and info.st_size == len(raw))
        _require(os.pread(fd, len(raw), 0) == raw)
        self._verify()

    def _receipt(self, transport):
        inputs, plan = self._inputs, self._plan
        facts = inputs.generation
        _require(facts.chain_id == plan.chain_id and facts.lane_count == plan.lane_count
                 and len(facts.accounts) == plan.account_count)
        peers = []
        for index, role in enumerate(inputs.roles):
            root = self._namespace.root / 'storage' / f'peer{index}'
            _require(role.peer_id == f'peer{index}' and role.block_store == root / 'kura')
            addresses = inputs.node_transport(index)
            _endpoint(addresses[0], plan.bind_host, plan.base_p2p_port + index, True)
            _endpoint(addresses[1], plan.public_host, plan.base_p2p_port + index, True)
            _endpoint(addresses[2], plan.bind_host, plan.base_api_port + index, True)
            _endpoint(role.torii_url, plan.public_host, plan.base_api_port + index, False)
            runtime_paths = inputs.node_runtime_paths(index)
            _require(runtime_paths == tuple((field, str(root / relative)) for field, relative in _RUNTIME_PATHS))
            peers.append(GeneratedPeer(role, *addresses, root / 'state', runtime_paths))
        return GenerationReceipt(plan, self._image_binding[3], transport.process,
            inputs.anchors_sha256, len(transport.stdout), inputs.genesis_hash, inputs.context_id,
            inputs.network_id, facts.genesis_public_key, facts.chain_discriminant,
            tuple(peers), facts.accounts, facts.artifacts)

    def generate(self, development_seed: str) -> GeneratedInputs:
        """Generate once from a runtime-only private 32-byte hexadecimal seed."""
        try:
            _require(self._phase == 'admitted' and type(development_seed) is str
                     and re.fullmatch(r'[0-9a-f]{64}', development_seed) and development_seed != '0' * 64)
            self._phase = 'generating'
            self._verify()
            self._namespace.empty(self._namespace.root)
            plan = self._plan
            self._seed_pipe = DevelopmentSeedPipe(development_seed)
            seed_fd = self._seed_pipe.fd
            argv = (str(self._image_binding[0]), '--ui-mode', 'plain', 'localnet', '--peers', '4',
                '--consensus-mode', 'npos', '--scaling-lanes', str(plan.lane_count),
                '--scaling-accounts', str(plan.account_count), '--seed-fd', str(seed_fd),
                '--chain-id', plan.chain_id, '--bind-host', plan.bind_host,
                '--public-host', plan.public_host, '--base-api-port', str(plan.base_api_port),
                '--base-p2p-port', str(plan.base_p2p_port), '--out-dir', str(self._namespace.root))
            transport = self._commands.run('generator', argv, (seed_fd,), MAX_ANCHORS_BYTES)
            self._seed_pipe.validate()
            self._seed_pipe.close()
            self._seed_pipe = None
            self._capture_anchor(transport.stdout)
            self._inputs = ReadinessInputs(self._namespace.root, hashlib.sha256(transport.stdout).hexdigest(), plan.lane_count)
            _require(self._inputs.anchor_bytes() == transport.stdout)
            initial_stores = self._namespace.admit_stores()
            receipt = self._receipt(transport)
            self._verify()
            self._namespace.validate_initial_stores(initial_stores)
            self._generated = GeneratedInputs(self, self._inputs, receipt)
            self._phase = 'generated'
            return self._generated
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def validate(self):
        """Validate this generation's original public receipt and retained inputs."""
        try:
            _require(self._phase == 'generated' and self._generated is not None)
            self._verify()
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Reap only original generator children; keep generated inputs retained."""
        self._phase = 'failed'
        return self._commands.cleanup(deadline_ns)

    def close(self):
        """Release only generation-owned descriptors after its children are reaped."""
        try:
            self._close()
        except BaseException as error:
            if self._phase != 'closed': self._phase = 'failed'
            _failure(error)

    def _close(self):
        if self._phase == 'closed': return
        for child in self._commands._children:
            self._commands._bound(child)
            _require(child.process.poll() is not None)
            self._commands._bound(child)
        self._phase = 'closed'
        try:
            if self._inputs is not None: self._inputs.close()
        finally:
            try:
                if self._anchor is not None:
                    fd, self._anchor = self._anchor[0], None
                    os.close(fd)
            finally:
                try:
                    self._namespace.close()
                finally:
                    if self._seed_pipe is not None:
                        self._seed_pipe.close()
                        self._seed_pipe = None
