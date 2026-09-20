"""Retain the fixed native generator's original readiness input census.

Requires Python 3.11+. The caller pins the native generator's exact anchor-report
digest after matching its successful stdout with genesis-anchors.json. This owner
checks that native JSON contract and retains its original files and lexical parent
edges. It does not decode .nrt, derive a context ID, authenticate a generator, or
replace the fixed launcher's independently retained runtime dependency closure.
"""
from __future__ import annotations

from dataclasses import dataclass
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tomllib
from urllib.parse import urlsplit

from resource_process import _directory_identity, _file_identity

ANCHORS_SCHEMA = 'iroha.sumeragi_v2.scaling.genesis_anchors.v1'
MAX_ANCHORS_BYTES = 32768
MAX_CONFIG_BYTES = 1024 * 1024
MAX_TOTAL_INPUT_BYTES = 512 * 1024 * 1024
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_FILE_FLAGS = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
_HASH = re.compile(r'hash:[0-9A-F]{64}#[0-9A-F]{4}')
_SHA = re.compile(r'[0-9a-f]{64}')
_BLS = re.compile(r'ea0130[0-9A-F]{96}')
_ED25519 = re.compile(r'ed0120[0-9A-F]{64}')
_PUBLIC_GENESIS_NAMES = frozenset(('genesis.json', 'genesis.signed.nrt',
    'genesis-context.nrt', 'genesis.expected_hash', 'genesis-anchors.json'))


class ReadinessError(ValueError):
    """A fixed failure code that never includes child output or input contents."""


def _require(value, code):
    if not value:
        raise ReadinessError(code)


def _public_failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise ReadinessError('readiness_failed') from None


def _fields(value, fields):
    _require(type(value) is dict and value.keys() == set(fields), 'readiness_input_fields')


def _integer(value, minimum, maximum):
    _require(type(value) is int and minimum <= value <= maximum, 'readiness_input_integer')
    return value


def _object(pairs):
    value = {}
    for key, item in pairs:
        _require(key not in value, 'readiness_input_duplicate_key')
        value[key] = item
    return value


def _number(value):
    _require(len(value) <= 10 and value.isascii() and value.isdigit(), 'readiness_input_integer')
    return int(value)


def _invalid_number(_):
    raise ReadinessError('readiness_input_integer')


def _hash(value):
    _require(type(value) is str and _HASH.fullmatch(value)
             and int(value[68], 16) & 1 == 1, 'readiness_input_hash')
    return value


def _path(path):
    _require(type(path) is type(Path('/')) and path.anchor == '/'
             and str(path) == os.path.abspath(path) and 1 <= len(path.parts) - 1 <= 64
             and len(os.fsencode(path)) <= 4096, 'readiness_input_path')


def _url(value):
    _require(type(value) is str and value.isascii() and 1 <= len(value) <= 2048,
             'readiness_input_url')
    parsed = urlsplit(value)
    _require(parsed.scheme in ('http', 'https') and parsed.hostname is not None
             and parsed.username is None and parsed.password is None
             and parsed.port is not None and 1 <= parsed.port <= 65535
             and parsed.path == '/' and not parsed.query and not parsed.fragment,
             'readiness_input_url')
    return value


def _primary_path(value, store):
    _require(type(value) is str and '\x00' not in value, 'readiness_primary_path')
    path = Path(value)
    _path(path)
    _require(str(path) == value and store in path.parents, 'readiness_primary_path')
    return path


@dataclass(frozen=True, slots=True)
class ReadinessRole:
    """Native role and client configuration selected from the original census."""
    peer_id: str
    node_config: Path
    block_store: Path
    node_public_key: str
    client_config: Path
    client_config_sha256: str
    torii_url: str
    primary_block_store: Path
    primary_merge_log: Path


@dataclass(frozen=True, slots=True)
class GeneratedAccount:
    """Original native account projection with its exact config identity."""
    index: int
    account_id: str
    config: str
    config_sha256: str


@dataclass(frozen=True, slots=True)
class GeneratedArtifact:
    """Original native artifact census row, without private file contents."""
    path: str
    sha256: str
    bytes: int


@dataclass(frozen=True, slots=True)
class GenerationFacts:
    """Public native generation facts authenticated by retained original files."""
    chain_id: str
    lane_count: int
    genesis_public_key: str
    chain_discriminant: int
    accounts: tuple[GeneratedAccount, ...]
    artifacts: tuple[GeneratedArtifact, ...]


@dataclass(frozen=True, slots=True)
class _Anchors:
    genesis_hash: str
    context_id: str
    network_id: str
    lane_count: int
    roles: tuple[ReadinessRole, ...]


class ReadinessInputs:
    """One original anchor report and every immutable file named by its census.

    The caller must keep this context open through readiness, load, stopped proof
    replay and final checks. Mutable store namespaces are separately retained by
    PeerLaunchInputs. Neither namespace changes nor identical-byte replacements
    refresh the original file ownership recorded here.
    """

    def __init__(self, directory: Path, anchors_sha256: str, lane_count: int):
        _require(not hasattr(self, '_directory'), 'readiness_inputs_readmission')
        _path(directory)
        _require(type(anchors_sha256) is str and _SHA.fullmatch(anchors_sha256), 'readiness_input_digest')
        _require(type(lane_count) is int and lane_count in (1, 4), 'readiness_input_lanes')
        self._directory, self._anchors_sha256 = directory, anchors_sha256
        self._directories, self._files, self._file_flags = {}, {}, {}
        self._total, self._failed = 0, False
        try:
            root_fd = self._retain_directory(directory)
            root_info = os.fstat(root_fd)
            _require(root_info.st_uid == os.geteuid() and stat.S_IMODE(root_info.st_mode) == 0o700,
                     'readiness_input_directory_owner')
            fd, raw = self._retain_file('genesis-anchors.json', anchors_sha256, None, MAX_ANCHORS_BYTES, True)
            _require(raw.startswith(b'{') and raw.endswith(b'}\n') and b'\n' not in raw[:-1], 'readiness_anchor_framing')
            value = json.loads(raw.decode('utf-8'), object_pairs_hook=_object, parse_int=_number,
                               parse_float=_invalid_number, parse_constant=_invalid_number)
            _fields(value, ('schema', 'version', 'lane_count', 'genesis_hash', 'context_id', 'network_id',
                            'consensus_mode', 'chain_id', 'genesis_public_key', 'chain_discriminant',
                            'peers', 'accounts', 'artifacts'))
            _require(value['schema'] == ANCHORS_SCHEMA and type(value['version']) is int
                     and value['version'] == 1 and type(value['lane_count']) is int
                     and value['lane_count'] == lane_count and value['consensus_mode'] == 'npos'
                     and type(value['chain_id']) is str and 0 < len(value['chain_id']) <= 2048, 'readiness_anchor_scope')
            genesis, context, network = (_hash(value[name]) for name in ('genesis_hash', 'context_id', 'network_id'))
            _require(network == genesis, 'readiness_anchor_network')
            genesis_key = value['genesis_public_key']
            _require(type(genesis_key) is str and _ED25519.fullmatch(genesis_key),
                     'readiness_genesis_public_key')
            chain_discriminant = _integer(value['chain_discriminant'], 0, 65535)
            accounts, peers, artifacts = value['accounts'], value['peers'], value['artifacts']
            _require(type(accounts) is list and 4 <= len(accounts) <= 64 and len(accounts) % 4 == 0
                     and type(peers) is list and len(peers) == 4 and type(artifacts) is list,
                     'readiness_anchor_cohort')
            expected = {'genesis.json', 'genesis.signed.nrt', 'genesis-context.nrt',
                        'genesis.expected_hash', 'client.toml'}
            expected.update(f'peer{i}.toml' for i in range(4))
            expected.update(f'peer{i}-client.toml' for i in range(4))
            account_ids = set()
            for index, account in enumerate(accounts):
                _fields(account, ('index', 'account_id', 'config'))
                _require(type(account['index']) is int and account['index'] == index
                         and account['config'] == f'workload-account-{index:02}.toml'
                         and type(account['account_id']) is str and 0 < len(account['account_id']) <= 2048
                         and account['account_id'] not in account_ids, 'readiness_anchor_account')
                account_ids.add(account['account_id'])
                expected.add(account['config'])
            _require(len(artifacts) == len(expected), 'readiness_anchor_census')
            records, toml = {}, {}
            for item in artifacts:
                _fields(item, ('path', 'sha256', 'bytes'))
                name = item['path']
                _require(type(name) is str and name in expected and name not in records
                         and type(item['sha256']) is str and _SHA.fullmatch(item['sha256']),
                         'readiness_anchor_artifact')
                limit = MAX_CONFIG_BYTES if name.endswith('.toml') else (
                    1024 if name == 'genesis.expected_hash' else 64 * 1024 * 1024)
                length = _integer(item['bytes'], 1, limit)
                artifact_fd, contents = self._retain_file(name, item['sha256'], length, limit, name.endswith('.toml'))
                records[name] = item['sha256']
                if contents is not None:
                    config = tomllib.loads(contents.decode('utf-8'))
                    _require('extends' not in config, 'readiness_indirect_config')
                    toml[name] = config
            _require(records.keys() == expected, 'readiness_anchor_census')
            roles, keys, endpoints = [], set(), set()
            chain = toml['peer0.toml'].get('chain')
            _require(type(chain) is str and chain == value['chain_id'], 'readiness_config_chain')
            for index, peer in enumerate(peers):
                _fields(peer, ('role', 'node_public_key', 'torii_url', 'config', 'client_config',
                               'primary_block_store', 'primary_merge_log'))
                _require(peer['role'] == f'peer{index}' and peer['config'] == f'peer{index}.toml'
                         and peer['client_config'] == f'peer{index}-client.toml'
                         and type(peer['node_public_key']) is str and _BLS.fullmatch(peer['node_public_key']),
                         'readiness_anchor_peer')
                key, endpoint = peer['node_public_key'], _url(peer['torii_url'])
                _require(key not in keys and endpoint not in endpoints, 'readiness_duplicate_peer')
                keys.add(key); endpoints.add(endpoint)
                node, client = toml[peer['config']], toml[peer['client_config']]
                _require(node.get('public_key') == key and node.get('chain') == chain
                         and type(node.get('kura')) is dict and type(node['kura'].get('store_dir')) is str,
                         'readiness_node_config_binding')
                store = Path(node['kura']['store_dir'])
                _path(store)
                _require(str(store) == node['kura']['store_dir'], 'readiness_node_config_binding')
                # Native actual::LaneConfigEntry owns this geometry. These
                # descendants are intentionally absent before daemon startup.
                primary_store = _primary_path(peer['primary_block_store'], store)
                primary_log = _primary_path(peer['primary_merge_log'], store)
                _require(primary_store != primary_log and primary_store not in primary_log.parents
                         and primary_log not in primary_store.parents, 'readiness_primary_path_overlap')
                _require(client.get('chain') == chain and client.get('network_id') == network
                         and 'network_id_file' not in client and client.get('torii_url') == endpoint,
                         'readiness_client_config_binding')
                roles.append(ReadinessRole(peer['role'], directory / peer['config'], store, key,
                    directory / peer['client_config'], records[peer['client_config']], endpoint,
                    primary_store, primary_log))
            self._anchors = _Anchors(genesis, context, network, lane_count, tuple(roles))
            self._generation = GenerationFacts(chain, lane_count, genesis_key, chain_discriminant,
                tuple(GeneratedAccount(account['index'], account['account_id'], account['config'],
                                       records[account['config']]) for account in accounts),
                tuple(GeneratedArtifact(item['path'], item['sha256'], item['bytes']) for item in artifacts))
            _require(all(path != role.block_store and role.block_store not in path.parents
                         for path in (directory / name for name in self._files)
                         for role in roles), 'readiness_input_inside_store')
            self._store_roots = set()
            for role in roles:
                if directory in role.block_store.parents:
                    self._store_roots.add(role.block_store.relative_to(directory).parts[0])
            self.validate()
        except BaseException as error:
            self.close()
            _public_failure(error)

    @property
    def genesis_hash(self): return self._anchors.genesis_hash

    @property
    def context_id(self): return self._anchors.context_id

    @property
    def network_id(self): return self._anchors.network_id

    @property
    def genesis_public_key(self): return self.generation.genesis_public_key

    @property
    def chain_discriminant(self): return self.generation.chain_discriminant

    @property
    def anchors_sha256(self): return self._anchors_sha256

    @property
    def roles(self): return self._anchors.roles

    @property
    def role_bindings(self):
        return tuple((role.peer_id, role.node_config, role.block_store) for role in self.roles)

    def _retain_directory(self, path):
        current, parent = Path('/'), None
        for index, name in enumerate(path.parts):
            current = Path('/') if index == 0 else current / name
            retained = self._directories.get(current)
            if retained is None:
                _require(len(self._directories) < 128, 'readiness_directory_bound')
                fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode), 'readiness_ancestor_not_directory')
                    retained = (fd, parent, name, _directory_identity(info))
                    self._directories[current] = retained
                except BaseException:
                    os.close(fd)
                    raise
            parent = retained[0]
        return parent

    def _retain_file(self, name, expected_sha, expected_bytes, limit, contents):
        parent = self._directories[self._directory][0]
        fd = os.open(name, _FILE_FLAGS, dir_fd=parent)
        try:
            info = os.fstat(fd)
            _require(3 <= fd <= 65535 and stat.S_ISREG(info.st_mode)
                     and info.st_uid == os.geteuid() and info.st_nlink == 1
                     and stat.S_IMODE(info.st_mode) in (0o400, 0o600)
                     and 0 < info.st_size <= limit
                     and (expected_bytes is None or info.st_size == expected_bytes), 'readiness_input_file')
            _require(self._total + info.st_size <= MAX_TOTAL_INPUT_BYTES, 'readiness_input_total_bound')
            identity, digest, offset = _file_identity(info), hashlib.sha256(), 0
            body = bytearray() if contents else None
            while offset < info.st_size:
                block = os.pread(fd, min(65536, info.st_size - offset), offset)
                _require(bool(block), 'readiness_input_truncated')
                digest.update(block)
                if body is not None: body.extend(block)
                offset += len(block)
            _require(digest.hexdigest() == expected_sha and _file_identity(os.fstat(fd)) == identity,
                     'readiness_input_changed')
            result = bytes(body) if body is not None else None
            flags = (fcntl.fcntl(fd, fcntl.F_GETFL), fcntl.fcntl(fd, fcntl.F_GETFD))
            _require(flags[0] & os.O_ACCMODE == os.O_RDONLY and flags[1] & fcntl.FD_CLOEXEC,
                     'readiness_input_descriptor_access')
            self._files[name] = (fd, parent, identity)
            self._file_flags[name] = flags
            self._total += info.st_size
            return fd, result
        except BaseException:
            os.close(fd)
            raise

    def _validate_directories(self):
        for fd, parent, name, identity in self._directories.values():
            _require(_directory_identity(os.fstat(fd)) == identity
                     and _directory_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity,
                     'readiness_input_directory_changed')

    def _validate_census(self):
        expected = self._files.keys() | self._store_roots
        found = set()
        root = self._directories[self._directory][0]
        with os.scandir(root) as entries:
            for entry in entries:
                _require(len(found) < len(expected) and entry.name in expected, 'readiness_input_census_changed')
                found.add(entry.name)
                if entry.name in self._store_roots:
                    _require(entry.is_dir(follow_symlinks=False), 'readiness_store_namespace_changed')
        _require(self._files.keys() <= found, 'readiness_input_census_changed')

    def validate(self):
        """Recheck every original retained descriptor and lexical named edge."""
        try:
            _require(not self._failed and len(self._files) >= 18, 'readiness_inputs_unavailable')
            self._validate_directories()
            for name, (fd, parent, identity) in self._files.items():
                _require(_file_identity(os.fstat(fd)) == identity
                         and _file_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity
                         and (fcntl.fcntl(fd, fcntl.F_GETFL), fcntl.fcntl(fd, fcntl.F_GETFD))
                         == self._file_flags[name],
                         'readiness_input_changed')
            self._validate_census()
            self._validate_directories()
        except BaseException as error:
            self._failed = True
            _public_failure(error)

    @property
    def generation(self) -> GenerationFacts:
        """Return immutable public facts only while original custody still holds."""
        self.validate()
        return self._generation

    @property
    def input_directory(self) -> Path:
        """Original lexical output root; this accessor never refreshes admission."""
        return self._directory

    def _original_bytes(self, name: str, limit: int) -> bytes:
        self.validate()
        fd, _, identity = self._files[name]
        length = os.fstat(fd).st_size
        _require(type(length) is int and 0 < length <= limit, 'readiness_input_bound')
        raw = os.pread(fd, length, 0)
        _require(len(raw) == length, 'readiness_input_truncated')
        self.validate()
        return raw

    def anchor_bytes(self) -> bytes:
        """Bounded bytes of the retained native anchor file for stdout equality."""
        try:
            return self._original_bytes('genesis-anchors.json', MAX_ANCHORS_BYTES)
        except BaseException as error:
            self._failed = True
            _public_failure(error)

    def node_runtime_paths(self, index: int) -> tuple[tuple[str, str], ...]:
        """Project the fixed native producer's explicit role-owned writable paths.

        Native effective-config validation owns resolved defaults, including PoR
        VRF/drand derivations and absent optional persisted subsystems.
        """
        try:
            _integer(index, 0, 3)
            config = tomllib.loads(self._original_bytes(
                self.roles[index].node_config.name, MAX_CONFIG_BYTES).decode('utf-8'))
            fields = ('kura.store_dir', 'snapshot.store_dir', 'soracloud_runtime.state_dir',
                'tiered_state.cold_store_root', 'tiered_state.da_store_root', 'streaming.session_store_dir',
                'network.soranet_handshake.pow.revocation_store_path', 'torii.data_dir',
                'torii.da_ingest.replay_cache_store_dir', 'torii.da_ingest.manifest_store_dir',
                'sorafs.discovery.replay_checkpoint_path', 'sorafs.storage.data_dir', 'sorafs.por.state_dir')
            values = []
            for field in fields:
                value = config
                for component in field.split('.'):
                    _require(type(value) is dict, 'readiness_runtime_path_missing')
                    value = value.get(component)
                _require(type(value) is str and 0 < len(os.fsencode(value)) <= 4096,
                         'readiness_runtime_path_invalid')
                values.append((field, value))
            self.validate()
            return tuple(values)
        except BaseException as error:
            self._failed = True
            _public_failure(error)

    def node_transport(self, index: int) -> tuple[str, str, str]:
        """Project original native address literals, without parsing Norito data."""
        try:
            _integer(index, 0, 3)
            raw = self._original_bytes(self.roles[index].node_config.name, MAX_CONFIG_BYTES)
            config = tomllib.loads(raw.decode('utf-8'))
            _require(type(config.get('network')) is dict and type(config.get('torii')) is dict,
                     'readiness_transport_missing')
            values = (config['network'].get('address'), config['network'].get('public_address'),
                      config['torii'].get('address'))
            _require(all(type(value) is str and 1 <= len(value) <= 2048 for value in values),
                     'readiness_transport_invalid')
            self.validate()
            return values
        except BaseException as error:
            self._failed = True
            _public_failure(error)

    def client_fd(self, index):
        """Borrow the original read-only client descriptor for one fixed role."""
        _integer(index, 0, 3)
        self.validate()
        return self._files[self.roles[index].client_config.name][0]

    def public_descriptor(self, name: str) -> int:
        """Borrow one of five original public genesis files without reopening it.

        The caller immediately duplicates the read-only descriptor, owns that
        duplicate, and retains its own complete admission before this owner
        closes. No private TOML is exportable; the caller still enforces its
        original trial deadline and matches the native generation receipt.
        """
        try:
            _require(type(name) is str and name in _PUBLIC_GENESIS_NAMES,
                     'readiness_public_descriptor_name')
            self.validate()
            original = self._files[name]
            self.validate()
            fd, parent, identity = original
            _require(self._files[name] is original
                     and _file_identity(os.fstat(fd)) == identity
                     and _file_identity(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity
                     and (fcntl.fcntl(fd, fcntl.F_GETFL), fcntl.fcntl(fd, fcntl.F_GETFD))
                     == self._file_flags[name], 'readiness_public_descriptor_changed')
            return fd
        except BaseException as error:
            self._failed = True
            _public_failure(error)

    def close(self):
        """Release owned descriptors only; never remove an original input."""
        self._failed = True
        failures = []
        def close_original(fd, identity, flags=None):
            try:
                info = os.fstat(fd)
                _require((info.st_dev, info.st_ino) == identity[:2]
                         and (flags is None or (fcntl.fcntl(fd, fcntl.F_GETFL),
                                               fcntl.fcntl(fd, fcntl.F_GETFD)) == flags),
                         'readiness_descriptor_reused')
                os.close(fd)
            except (OSError, ReadinessError) as error:
                # A borrowed descriptor may have been closed and its number
                # reused. Preserve that foreign resource and fail the owner.
                failures.append(error)
        while self._files:
            name, (fd, _, identity) = self._files.popitem()
            flags = self._file_flags.pop(name, None)
            close_original(fd, identity, flags)
        while self._directories:
            fd, _, _, identity = self._directories.popitem()[1]
            close_original(fd, identity)
        if failures: _public_failure(failures[0])

    def __enter__(self): return self

    def __exit__(self, *_): self.close()
