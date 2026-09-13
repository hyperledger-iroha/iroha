"""Bind benchmark TCP listeners to kernel-observed process lifetimes.

This owner verifies endpoint attribution, not packet completeness or network
bytes. A complete capture and its loss counters remain separate prerequisites.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any

MAX_OUTPUT_BYTES = 1024 * 1024
MAX_PEERS = 68
LSOF = Path('/usr/sbin/lsof')


class ListenerObservationError(ValueError):
    """A listener cannot be joined to its exact benchmark process."""


def require(condition: bool, message: str) -> None:
    """Reject an incomplete or contradictory observation."""
    if not condition:
        raise ListenerObservationError(message)


def parse_listener_output(raw: bytes, pids: tuple[int, ...]) -> list[dict[str, Any]]:
    """Decode lsof's explicit NUL fields and exact LISTEN state, without names.

    The native lsof manual specifies newline-delimited process/file sets even
    with the NUL field terminator. TCP queue counts are observations, not a
    requirement that a listening socket have no pending connection.
    """
    require(type(raw) is bytes and 0 < len(raw) <= MAX_OUTPUT_BYTES,
            'listener output is empty or exceeds its bound')
    require(raw.endswith(b'\0\n'), 'listener output ended mid-record')
    pid = None
    seen_processes: set[int] = set()
    seen_descriptors: set[tuple[int, int]] = set()
    result = []
    for record in raw.split(b'\n')[:-1]:
        require(record.endswith(b'\0'), 'listener record lacks its NUL terminator')
        fields = record[:-1].split(b'\0')
        require(fields and all(fields), 'listener record has an empty field')
        if fields[0].startswith(b'p'):
            require(len(fields) == 1 and re.fullmatch(rb'p[1-9][0-9]*', fields[0]) is not None,
                    'malformed process record')
            pid = int(fields[0][1:])
            require(pid in pids and pid not in seen_processes, 'foreign or repeated process record')
            seen_processes.add(pid)
            continue
        require(pid is not None and fields[0].startswith(b'f'), 'socket lacks its process owner')
        values = {}
        for field in fields:
            key, value = field[:1], field[1:]
            if key == b'T':
                subtype, separator, value = value.partition(b'=')
                require(separator == b'=' and subtype in (b'ST', b'QR', b'QS'),
                        'unexpected TCP information field')
                key += subtype
            require(key in (b'f', b't', b'P', b'n', b'TST', b'TQR', b'TQS') and key not in values,
                    'unknown or duplicate socket field')
            values[key] = value
        require(set(values) >= {b'f', b't', b'P', b'n', b'TST'}, 'incomplete socket fields')
        require(re.fullmatch(rb'(0|[1-9][0-9]*)', values[b'f']) is not None,
                'invalid socket descriptor')
        fd = int(values[b'f'])
        require((pid, fd) not in seen_descriptors, 'repeated socket descriptor')
        seen_descriptors.add((pid, fd))
        require(values[b't'] == b'IPv4' and values[b'P'] == b'TCP' and values[b'TST'] == b'LISTEN',
                'listener is not IPv4 TCP in LISTEN state')
        for key in (b'TQR', b'TQS'):
            require(key not in values or re.fullmatch(rb'(0|[1-9][0-9]*)', values[key]) is not None,
                    'invalid TCP queue count')
        match = re.fullmatch(rb'127\.0\.0\.1:([1-9][0-9]*)', values[b'n'])
        require(match is not None, 'listener is not exact loopback IPv4')
        port = int(match.group(1))
        require(1 <= port <= 65535, 'listener port exceeds u16')
        result.append({'pid': pid, 'fd': fd, 'address': '127.0.0.1', 'port': port, 'transport': 'tcp'})
    require(seen_processes == set(pids) and {row['pid'] for row in result} == set(pids),
            'at least one declared process has no observed listener')
    require(len({(row['pid'], row['port']) for row in result}) == len(result),
            'a process has duplicate listeners for one declared port')
    return sorted(result, key=lambda row: (row['pid'], row['port'], row['fd']))


def _utility_identity() -> tuple[tuple[int, ...], str]:
    before = LSOF.lstat()
    require(stat.S_ISREG(before.st_mode) and not LSOF.is_symlink()
            and 0 < before.st_size < 16 * 1024 * 1024, 'lsof is not a bounded regular executable')
    with LSOF.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    after = LSOF.lstat()
    fields = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    identity = lambda info: tuple(getattr(info, key) for key in fields)
    require(identity(before) == identity(after), 'lsof changed during identity capture')
    return identity(after), digest


class DarwinListenerReader:
    """Query only explicitly selected PIDs through the native kernel utility."""

    def __init__(self) -> None:
        require(sys.platform == 'darwin', 'native listener adapter requires Darwin')
        self.utility = _utility_identity()

    def read(self, pids: tuple[int, ...]) -> dict[str, Any]:
        """Retain bounded raw output; reject missing owners and utility changes."""
        require(type(pids) is tuple and 1 <= len(pids) <= MAX_PEERS
                and all(type(pid) is int and 1 < pid < 1 << 31 for pid in pids)
                and len(set(pids)) == len(pids), 'invalid selected process set')
        require(_utility_identity() == self.utility, 'listener utility changed')
        command = [str(LSOF), '-nP', '-a', '-p', ','.join(map(str, pids)),
                   '-iTCP', '-sTCP:LISTEN', '-F0pftPnT']
        started = time.monotonic_ns()
        # Temporary descriptors avoid an unbounded pipe buffer in the caller.
        with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
            try:
                completed = subprocess.run(command, stdin=subprocess.DEVNULL,
                                           stdout=output, stderr=errors, timeout=10, check=False)
            except subprocess.TimeoutExpired as error:
                raise ListenerObservationError('listener utility exceeded its observation deadline') from error
            require(completed.returncode == 0, 'listener utility did not return a complete observation')
            require(output.tell() <= MAX_OUTPUT_BYTES and errors.tell() == 0,
                    'listener utility output is oversized or has diagnostics')
            output.seek(0)
            raw = output.read(MAX_OUTPUT_BYTES + 1)
        rows = parse_listener_output(raw, pids)
        require(_utility_identity() == self.utility, 'listener utility changed during observation')
        return {'started_monotonic_ns': started, 'finished_monotonic_ns': time.monotonic_ns(),
                'command': command, 'utility_sha256': self.utility[1],
                'raw_utf8': raw.decode('ascii'), 'raw_sha256': hashlib.sha256(raw).hexdigest(),
                'listeners': rows}


def declared_listener_rows(document: dict[str, Any], *, session: dict[str, Any],
                           network_id: Any, participants: int, scope: Any) -> list[dict[str, Any]]:
    """Join the Rust port document to the exact session, topology and process order."""
    import private_settlement_session_control as control
    import private_settlement_capture_split as capture
    identity = control.identity(session)
    fields = control.IDENTITY_FIELDS | {'kind', 'network_id', 'participants', 'groups', 'peers'}
    control.exact(document, fields, 'network port document')
    require(type(participants) is int and participants in (2, 3, 4, 8, 16),
            'unsupported participant topology')
    require(control.canonical({key: document[key] for key in control.IDENTITY_FIELDS})
            == control.canonical(identity)
            and control.canonical(document['network_id']) == control.canonical(network_id)
            and type(document['participants']) is int and document['participants'] == participants
            and document['kind'] == 'benchmark_network_ports', 'port document identity differs')
    count = 4 * (participants + 1)
    require(type(document['peers']) is list and len(document['peers']) == count
            and len(scope.declarations) == count + 2, 'incomplete topology or kernel process scope')
    groups = {'torii': [], 'public_p2p': [], 'restricted_p2p': []}
    result = []
    for index, peer in enumerate(document['peers']):
        lane, validator = divmod(index, 4)
        role = 'global_validator' if lane == 0 else 'dataspace_validator'
        ordinal = None if lane == 0 else lane - 1
        visibility = 'public' if lane in (0, 1) else 'restricted'
        control.exact(peer, {'peer_index', 'pid', 'role', 'dataspace_ordinal',
                            'validator_ordinal', 'torii', 'p2p'}, 'peer endpoint')
        declaration = scope.declarations[index + 1]
        require(type(peer['peer_index']) is int and peer['peer_index'] == index
                and type(peer['pid']) is int and peer['pid'] == declaration['pid']
                and peer['role'] == role and peer['dataspace_ordinal'] == ordinal
                and (peer['dataspace_ordinal'] is None or type(peer['dataspace_ordinal']) is int)
                and type(peer['validator_ordinal']) is int and peer['validator_ordinal'] == validator
                and declaration['image'] == 'validator'
                and declaration['label'] == f'process-{index + 1:03}-{role}',
                'endpoint differs from its ordered committee process')
        for kind in ('torii', 'p2p'):
            endpoint = peer[kind]
            control.exact(endpoint, {'address', 'port', 'transport'}
                          | ({'visibility'} if kind == 'p2p' else set()), 'TCP endpoint')
            require(type(endpoint['port']) is int and 1 <= endpoint['port'] <= 65535
                    and endpoint['address'] == '127.0.0.1' and endpoint['transport'] == 'tcp'
                    and (kind != 'p2p' or endpoint['visibility'] == visibility),
                    'endpoint transport or visibility differs')
            result.append({'pid': peer['pid'], **{key: endpoint[key] for key in ('address', 'port', 'transport')}})
            groups['torii' if kind == 'torii' else visibility + '_p2p'].append(endpoint['port'])
    for ports in groups.values():
        ports.sort()
    expected = capture.canonical_port_manifest_document(groups)
    require(control.canonical(document['groups']) == control.canonical(expected),
            'grouped port inventory differs from exact peer endpoints')
    require(len({row['pid'] for row in result}) == count, 'port document repeats a process owner')
    return result


def observe_declared_listeners(scope: Any, expected: list[dict[str, Any]],
                               reader: Any) -> dict[str, Any]:
    """Bracket listener attribution with the exact process/image lifetime scope.

    The caller derives the expected rows from the session's bound network-port
    document. Every validator must expose exactly its two declared TCP listeners.
    This observation cannot authenticate an unbound port declaration by itself.
    """
    require(type(expected) is list and 2 <= len(expected) <= 2 * MAX_PEERS,
            'invalid endpoint inventory size')
    for row in expected:
        require(type(row) is dict and set(row) == {'pid', 'address', 'port', 'transport'}
                and type(row['pid']) is int and 1 < row['pid'] < 1 << 31
                and type(row['port']) is int and 1 <= row['port'] <= 65535
                and row['address'] == '127.0.0.1' and row['transport'] == 'tcp',
                'invalid declared endpoint')
    require(len({row['port'] for row in expected}) == len(expected), 'declared ports overlap')
    pids = tuple(sorted({row['pid'] for row in expected}))
    require(all(sum(row['pid'] == pid for row in expected) == 2 for pid in pids),
            'validator must declare exactly Torii and P2P listeners')
    before = scope.observe()
    identities = {row['identity']['pid']: row['identity'] for row in before['processes']}
    require(set(pids) <= set(identities), 'endpoint owner is absent from the exact process scope')
    observed = reader.read(pids)
    actual = [{key: row[key] for key in ('pid', 'address', 'port', 'transport')}
              for row in observed['listeners']]
    order = lambda row: (row['pid'], row['port'])
    require(sorted(actual, key=order) == sorted(expected, key=order),
            'kernel listener inventory differs from the declared endpoint owners')
    after = scope.observe()
    require([row['identity'] for row in before['processes']]
            == [row['identity'] for row in after['processes']], 'process identity changed around listener observation')
    return {'process_before': before, 'listener_observation': observed,
            'process_after': after, 'expected_endpoints': expected,
            'scope': 'kernel listener attribution only; packet completeness and byte totals unmeasured'}
