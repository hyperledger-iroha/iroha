"""Bounded launch input and secret transport for the fixed scaling command.

Only standard-library modules load before the bootstrap has authenticated the
private source and dependency packs. Launch data describes existing release
inputs; it never supplies executable commands or a qualification assertion.
"""
from dataclasses import dataclass, fields
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat

LAUNCH_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.launch.v1'
MAX_LAUNCH_BYTES = 64 * 1024
MAX_DESCRIPTOR = (1 << 20) - 1
_LAUNCH_FIELDS = frozenset(('schema', 'runtime_paths', 'python_dependencies',
    'plan', 'budget', 'evidence_root', 'runtime_root', 'worker_sources', 'identity'))


class LaunchInputError(ValueError):
    """Invalid launch data, without exposing runtime paths or secret material."""


def _require(value):
    if not value:
        raise LaunchInputError('fixed_scaling_launch_invalid')


def _digest(value):
    _require(type(value) is str and re.fullmatch('[a-f0-9]{64}', value))
    return value


def _path(value):
    _require(type(value) is str and 0 < len(value) <= 4096 and '\0' not in value)
    path = Path(value)
    _require(path.is_absolute() and str(path) == value
             and value == os.path.abspath(value) and len(path.parts) <= 64
             and len(os.fsencode(path)) <= 4096)
    return path


def _label(value):
    _require(type(value) is str and 0 < len(value) <= 512
             and all(32 <= ord(char) < 127 for char in value))
    return value


def _fields(value, names):
    _require(type(value) is dict and value.keys() == set(names))
    return value


def _pairs(rows):
    _require(len(rows) <= 32)
    value = {}
    for key, item in rows:
        _require(type(key) is str and 0 < len(key) <= 128 and key not in value)
        value[key] = item
    return value


def _reject_number(_):
    raise LaunchInputError('fixed_scaling_launch_invalid')


def load_launch_value(raw: bytes) -> dict:
    """Parse bounded ASCII JSON before importing any scaling runtime owner."""
    try:
        _require(type(raw) is bytes and 1 < len(raw) <= MAX_LAUNCH_BYTES and raw.isascii())
        stack, quoted, escaped, tokens, text_bytes = [], False, False, 0, 0
        for byte in raw:
            if quoted:
                text_bytes += 1
                _require(text_bytes <= 12 * 4096 + 1)
                if escaped:
                    escaped = False
                elif byte == 92:
                    escaped = True
                elif byte == 34:
                    quoted = False
            elif byte == 34:
                quoted, text_bytes = True, 0
                tokens += 1
            elif byte == 123:
                stack.append(125)
                _require(len(stack) <= 3)
                tokens += 1
            elif byte == 125:
                _require(bool(stack) and stack.pop() == byte)
                tokens += 1
            elif byte in (91, 93):
                _require(False)
            elif byte in (44, 58):
                tokens += 1
            _require(tokens <= 512)
        _require(not quoted and not escaped and not stack)
        value = json.loads(raw.decode('ascii'), object_pairs_hook=_pairs,
            parse_int=_reject_number, parse_float=_reject_number, parse_constant=_reject_number)
        _fields(value, _LAUNCH_FIELDS)
        _require(type(value['schema']) is str and value['schema'] == LAUNCH_SCHEMA)
        pending, nodes = [value], 0
        while pending:
            item = pending.pop()
            nodes += 1
            _require(nodes <= 128)
            if type(item) is dict:
                pending.extend(item.values())
            else:
                _require(type(item) is str and 0 < len(item) <= 4096)
        return value
    except (ValueError, TypeError, OverflowError, RecursionError):
        raise LaunchInputError('fixed_scaling_launch_invalid') from None


@dataclass(frozen=True, slots=True)
class LaunchInputs:
    """Typed inputs; actual source, runtime and file admission still must run."""
    runtime_paths: object
    python_dependencies: object
    plan_path: Path
    plan_sha256: str
    budget_path: Path
    budget_sha256: str
    evidence_root: Path
    runtime_root: Path
    worker_sources: Path
    machine_id: str
    storage_model: str
    source_revision: str


def decode_launch_input(raw: bytes) -> LaunchInputs:
    """Decode the single exact schema after bootstrap loads the verified owners."""
    from scaling_runtime_admission import ReleaseRuntimePaths
    from scaling_cli_bootstrap import PythonDependencyPaths
    try:
        value = load_launch_value(raw)
        runtime = _fields(value['runtime_paths'], (field.name for field in fields(ReleaseRuntimePaths)))
        converted = {}
        for field in fields(ReleaseRuntimePaths):
            item = runtime[field.name]
            if field.name.endswith('sha256'):
                converted[field.name] = _digest(item)
            elif field.name == 'python_entrypoint':
                _require(item == 'scripts/nexus/run_multilane_scaling_gate.py')
                converted[field.name] = item
            else:
                converted[field.name] = _path(item)
        dependency = _fields(value['python_dependencies'],
            ('source_root', 'bundle_root', 'inventory', 'inventory_sha256'))
        dependency_paths = PythonDependencyPaths(
            _path(dependency['source_root']), _path(dependency['bundle_root']),
            _path(dependency['inventory']), _digest(dependency['inventory_sha256']))
        plan = _fields(value['plan'], ('path', 'sha256'))
        budget = _fields(value['budget'], ('path', 'sha256'))
        identity = _fields(value['identity'], ('machine_id', 'storage_model', 'source_revision'))
        revision = identity['source_revision']
        _require(type(revision) is str and re.fullmatch('[a-f0-9]{40}|[a-f0-9]{64}', revision))
        return LaunchInputs(ReleaseRuntimePaths(**converted), dependency_paths,
            _path(plan['path']), _digest(plan['sha256']),
            _path(budget['path']), _digest(budget['sha256']),
            _path(value['evidence_root']), _path(value['runtime_root']),
            _path(value['worker_sources']), _label(identity['machine_id']),
            _label(identity['storage_model']), revision)
    except (ValueError, TypeError, OverflowError, RecursionError):
        raise LaunchInputError('fixed_scaling_launch_invalid') from None


def _fd_identity(info):
    return (info.st_dev, info.st_ino, stat.S_IFMT(info.st_mode), info.st_uid, info.st_gid)


def _descriptor_pin(fd, info):
    """Pin observable descriptor state so cleanup does not consume a replacement."""
    return (_fd_identity(info), fcntl.fcntl(fd, fcntl.F_GETFL),
            fcntl.fcntl(fd, fcntl.F_GETFD))


def _descriptor(fd):
    _require(type(fd) is int and 2 < fd <= MAX_DESCRIPTOR)
    info = os.fstat(fd)
    _require(info.st_uid == os.geteuid()
             and fcntl.fcntl(fd, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY)
    return info


def read_launch_descriptor(fd: int, expected_sha256: str) -> bytes:
    """Consume one inherited read-only regular-file descriptor, with an exact hash."""
    pin = None
    try:
        _digest(expected_sha256)
        before = _descriptor(fd)
        _require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                 and stat.S_IMODE(before.st_mode) in (0o400, 0o600)
                 and 1 < before.st_size <= MAX_LAUNCH_BYTES)
        pin = _descriptor_pin(fd, before)
        raw = bytearray()
        while len(raw) < before.st_size:
            chunk = os.pread(fd, min(4096, before.st_size - len(raw)), len(raw))
            _require(bool(chunk))
            raw.extend(chunk)
        result = bytes(raw)
        _require(hashlib.sha256(result).hexdigest() == expected_sha256)
        after = _descriptor(fd)
        _require(_descriptor_pin(fd, after) == pin and after.st_mode == before.st_mode
                 and after.st_nlink == before.st_nlink and after.st_size == before.st_size
                 and after.st_mtime_ns == before.st_mtime_ns and after.st_ctime_ns == before.st_ctime_ns)
        return result
    except (OSError, ValueError, TypeError, OverflowError):
        raise LaunchInputError('fixed_scaling_launch_invalid') from None
    finally:
        if pin is not None:
            try:
                # A changed slot is somebody else's handle: fail closed while
                # leaving that replacement open for its actual owner.
                _require(_descriptor_pin(fd, os.fstat(fd)) == pin)
                os.close(fd)
            except (OSError, ValueError):
                raise LaunchInputError('fixed_scaling_launch_invalid') from None


def read_seed_descriptor(fd: int) -> str:
    """Consume one ready nonblocking pipe containing 64 lowercase hex bytes and EOF."""
    pin = None
    try:
        before = _descriptor(fd)
        _require(stat.S_ISFIFO(before.st_mode) and not os.get_blocking(fd))
        pin = _descriptor_pin(fd, before)
        raw = os.read(fd, 65)
        _require(len(raw) == 64 and re.fullmatch(b'[a-f0-9]{64}', raw)
                 and os.read(fd, 1) == b'')
        result = raw.decode('ascii')
        _require(_descriptor_pin(fd, _descriptor(fd)) == pin and not os.get_blocking(fd))
        return result
    except (OSError, ValueError, TypeError, OverflowError):
        raise LaunchInputError('fixed_scaling_launch_invalid') from None
    finally:
        if pin is not None:
            try:
                # A changed slot is somebody else's handle: fail closed while
                # leaving that replacement open for its actual owner.
                _require(_descriptor_pin(fd, os.fstat(fd)) == pin)
                os.close(fd)
            except (OSError, ValueError):
                raise LaunchInputError('fixed_scaling_launch_invalid') from None
