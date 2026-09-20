#!/usr/bin/env python3
"""Render the fixed Taira epoch supervisor unit from admitted public paths.

Python 3.11+; no environment inputs or credential reads. This renderer validates
public structure only. The native installer must admit source/artifact hashes,
owner policy, file custody and the current process readiness independently.
Native exit 1 covers both command failure and finite worker-budget exhaustion;
it never proves transient failure. Bounded service restarts retain the same
explicit until-stopped policy and every original journal/transaction deadline.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import PurePosixPath, Path
import re
import stat

UNIT_NAME = 'iroha-taira-epoch-supervisor.service'
STATE_ROOT = '/var/lib/taira-epoch-supervisor'
JOURNAL_DIR = STATE_ROOT + '/journals'
SPEC_KEYS = frozenset({
    'schema_version', 'cli', 'admin_config', 'operator_key', 'policy', 'trust',
    'custody', 'journal_dir', 'timeout_ms',
})
GENERATION_FILES = {
    'admin_config': 'administrator.toml',
    'operator_key': 'http-operator.key',
    'policy': 'policy.json',
    'trust': 'trust.json',
    'custody': 'custody.json',
}


class UnitPolicyError(ValueError):
    """Public unit inputs or the closed native lifecycle contract are invalid."""


def require(value: bool, message: str) -> None:
    if not value:
        raise UnitPolicyError(message)


def canonical_path(value: object) -> str:
    require(isinstance(value, str) and bool(re.fullmatch(r'/[A-Za-z0-9_./:@+-]+', value)),
            'unit path must be an absolute literal without expansion or controls')
    path = PurePosixPath(value)
    require(str(path) == value and '..' not in path.parts and '//' not in value,
            'unit path must be normalized and direct')
    return value


def validate_spec(spec: object) -> dict:
    require(isinstance(spec, dict) and set(spec) == SPEC_KEYS, 'unit spec fields differ')
    require(type(spec['schema_version']) is int and spec['schema_version'] == 1,
            'unit spec requires schema version 1')
    require(type(spec['timeout_ms']) is int and 0 < spec['timeout_ms'] < 2 ** 64,
            'unit invocation requires an explicit positive finite timeout')
    for name in ('cli', *GENERATION_FILES, 'journal_dir'):
        canonical_path(spec[name])
    require(PurePosixPath(spec['cli']).name == 'iroha'
            and PurePosixPath(spec['cli']).parent.name == 'bin',
            'unit executable must be the independently admitted release bin/iroha')
    require(spec['journal_dir'] == JOURNAL_DIR, 'unit journal root differs')
    generation = PurePosixPath(spec['policy']).parent
    require(str(generation.parent) == STATE_ROOT + '/generations'
            and bool(re.fullmatch('[0-9a-f]{64}', generation.name)),
            'unit requires an immutable policy-digest generation')
    for name, filename in GENERATION_FILES.items():
        require(spec[name] == str(generation / filename), 'unit generation file differs: ' + name)
    return dict(spec)


def command(spec: object) -> tuple[str, ...]:
    """Return the sole argv grammar; paths remain public strings and are not read."""
    selected = validate_spec(spec)
    return (
        selected['cli'], '--config', selected['admin_config'],
        '--operator-private-key-file', selected['operator_key'],
        '--fee-payer', 'authority', 'taira', 'epoch-maintenance', 'supervise',
        '--policy', selected['policy'], '--trust', selected['trust'],
        '--custody', selected['custody'], '--journal-dir', JOURNAL_DIR,
        '--timeout-ms', str(selected['timeout_ms']),
    )


def systemd_argument(value: str) -> str:
    """Quote one systemd argv literal, including its specifier/environment grammar."""
    require(isinstance(value, str) and not any(ord(c) < 32 or ord(c) == 127 for c in value),
            'systemd argument contains a control character')
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%').replace('$', '$$') + '"'


def render(spec: object) -> bytes:
    """Render one direct native service; no service or credential is accessed."""
    arguments = ' '.join(systemd_argument(value) for value in command(spec))
    return (
        '[Unit]\nDescription=Taira epoch maintenance supervisor\n'
        'After=network-online.target\n'
        'StartLimitIntervalSec=300s\nStartLimitBurst=3\n\n'
        '[Service]\nType=exec\nUser=root\nGroup=root\nUMask=0077\n'
        f'WorkingDirectory={STATE_ROOT}\n'
        'NoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\n'
        f'ReadWritePaths={STATE_ROOT}\n'
        'Restart=on-failure\nRestartPreventExitStatus=3 4 7\nRestartSec=5s\n'
        'KillMode=control-group\nTimeoutStopSec=30s\n'
        f'ExecStart={arguments}\n\n'
        '[Install]\nWantedBy=multi-user.target\n'
    ).encode()


def unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, 'public unit spec has a duplicate field')
        result[key] = value
    return result


def load_public_spec(path: Path) -> dict:
    """Read one bounded regular public specification, never a credential path."""
    require(path.is_absolute() and path.parent.resolve() == path.parent,
            'public unit spec must have a direct absolute parent')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(fd, 'rb') as source:
        before = os.fstat(source.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                and 0 < before.st_size <= 16 * 1024,
                'public unit spec must be one bounded regular file')
        raw = source.read(16 * 1024 + 1)
        after = os.fstat(source.fileno())
        fields = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_uid', 'st_gid',
                  'st_size', 'st_mtime_ns', 'st_ctime_ns')
        require(len(raw) == before.st_size
                and all(getattr(before, name) == getattr(after, name) for name in fields),
                'public unit spec changed while reading')
    return validate_spec(json.loads(raw, object_pairs_hook=unique_object))


def publish(spec: object, output_path: Path) -> None:
    require(output_path.is_absolute() and output_path.name == UNIT_NAME
            and output_path.parent.resolve() == output_path.parent,
            'output must use the fixed unit name in an existing direct absolute directory')
    unit = render(spec)
    fd = os.open(output_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
    with os.fdopen(fd, 'wb') as output:
        os.fchmod(output.fileno(), 0o644)
        output.write(unit)
        output.flush()
        os.fsync(output.fileno())
    directory = os.open(output_path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--public-spec', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    publish(load_public_spec(args.public_spec), args.output)


if __name__ == '__main__':
    main()
