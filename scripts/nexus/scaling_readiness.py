"""Fixed four-peer native genesis readiness under the original trial deadline.

Requires retained generated inputs, a retained CLI image and its original runtime
closure supplied by the fixed launcher's input owner. The native command owns
canonical Norito, signature and quorum verification. This module owns invocation,
bounded pipes, exact report/input agreement and original child custody. It never
accepts a user command or treats an unsigned failure as readiness evidence.
"""
from __future__ import annotations

import base64
from dataclasses import dataclass
import hashlib
import json
import secrets
import time
from typing import Callable

from resource_process import ExecutableImage, PinnedProcess, ProcessIdentity
from scaling_command import BoundedCommand
from scaling_readiness_inputs import ReadinessError, ReadinessInputs, _require, _public_failure

MAX_REPORT_BYTES = 24 * 1024 * 1024
MAX_ATTESTATION_BYTES = 16 * 1024 * 1024
MAX_DIAGNOSTIC_BYTES = 16 * 1024
MAX_TRIAL_NS = 7200 * 1_000_000_000
RETRY_NS = 100_000_000
_FIELDS = frozenset(('version', 'state', 'reason', 'challenge', 'node_id',
                    'network_id', 'genesis_hash', 'context_id', 'attestation_norito_base64'))
_PENDING = frozenset(('consensus_uninitialized', 'genesis_uncommitted'))


def _remaining(end_ns: int) -> float:
    remaining = end_ns - time.monotonic_ns()
    _require(remaining > 0, 'readiness_deadline_exceeded')
    return remaining / 1_000_000_000


def _object(pairs):
    result = {}
    for key, value in pairs:
        _require(key not in result, 'readiness_report_duplicate_key')
        result[key] = value
    return result


def _number(value):
    _require(value == '1', 'readiness_report_number_invalid')
    return 1


def _invalid_number(_):
    raise ReadinessError('readiness_report_number_invalid')


@dataclass(frozen=True, slots=True)
class ReadyReceipt:
    """Native verified bytes bound to one original validator and fresh challenge."""
    peer_id: str
    process: ProcessIdentity
    node_id: str
    network_id: str
    genesis_hash: str
    context_id: str
    challenge: str
    cli_sha256: str
    cli_process: ProcessIdentity
    client_config_sha256: str
    anchors_sha256: str
    report_sha256: str
    attestation: bytes




class FourPeerReadiness:
    """One use of the fixed CLI, one original deadline, four exact ready receipts.

The launcher keeps this object through cleanup. Every created CLI child is
retained even if pinning or I/O fails. A timeout cannot return readiness or erase
an unreaped original child. Only typed pending reports permit another invocation.
"""

    def __init__(self, inputs: ReadinessInputs, image: ExecutableImage, reader,
                 trial_deadline_ns: int, verify_runtime: Callable[[], None]):
        _require(not hasattr(self, '_inputs'), 'readiness_readmission')
        _require(type(inputs) is ReadinessInputs and isinstance(image, ExecutableImage)
                 and callable(getattr(reader, 'sample', None)) and callable(verify_runtime), 'readiness_owner_invalid')
        _require(type(trial_deadline_ns) is int
                 and 0 < trial_deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS,
                 'readiness_deadline_invalid')
        inputs.validate()
        image.validate()
        self._inputs, self._image, self._reader = inputs, image, reader
        self._input_binding = (inputs, inputs._directory, inputs._anchors, inputs.anchors_sha256)
        self._image_binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
        self._end = trial_deadline_ns
        self._phase = 'admitted'
        self._challenges: set[str] = set()
        self._receipts: list[ReadyReceipt] = []
        self._guard: Callable[[], None] | None = None
        self._verify_runtime = verify_runtime
        self._commands = BoundedCommand(image, reader, trial_deadline_ns, self._verify)

    @property
    def trial_deadline_ns(self) -> int:
        """The constructor's absolute deadline; retries never replace it."""
        return self._end

    @property
    def role_bindings(self):
        """Original role, node-config and store tuples for launcher agreement."""
        return self._inputs.role_bindings

    def _verify_owners(self):
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids)
                 == self._image_binding, 'readiness_image_binding_changed')
        original, directory, anchors, digest = self._input_binding
        _require(self._inputs is original and self._inputs._directory == directory
                 and self._inputs._anchors is anchors and self._inputs.anchors_sha256 == digest,
                 'readiness_input_binding_changed')
        image.validate()
        self._inputs.validate()

    def _verify(self):
        _require(self._phase == 'busy', 'readiness_phase_invalid')
        _remaining(self._end)
        self._verify_owners()
        self._verify_runtime()
        _require(self._phase == 'busy', 'readiness_phase_invalid')
        self._guard()
        _require(self._phase == 'busy', 'readiness_phase_invalid')
        self._verify_owners()
        _remaining(self._end)

    def _invoke(self, index: int, challenge: str):
        self._verify()
        role = self._inputs.roles[index]
        timeout_ms = min(60000, (self._end - time.monotonic_ns()) // 1_000_000)
        _require(timeout_ms >= 1, 'readiness_deadline_exceeded')
        fd = self._inputs.client_fd(index)
        argv = (str(self._image.path), '--machine', '--config-fd', str(fd),
                '--config-source-path', str(role.client_config), '--output-format', 'json',
                'bridge', 'genesis-readiness', '--challenge', challenge,
                '--node-public-key', role.node_public_key,
                '--genesis-hash', self._inputs.genesis_hash[5:69],
                '--context-id', self._inputs.context_id[5:69],
                '--request-timeout-ms', str(timeout_ms))
        result = self._commands.run(role.peer_id, argv, (fd,), MAX_REPORT_BYTES)
        return result.stdout, result.process

    def _report(self, index, challenge, raw, identity, cli_identity):
        _require(type(raw) is bytes and 1 < len(raw) <= MAX_REPORT_BYTES
                 and raw.startswith(b'{') and raw.endswith(b'}\n') and b'\n' not in raw[:-1], 'readiness_report_framing')
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=_object, parse_int=_number,
                           parse_float=_invalid_number, parse_constant=_invalid_number)
        _require(type(value) is dict and value.keys() == _FIELDS, 'readiness_report_fields')
        role = self._inputs.roles[index]
        expected = {'challenge': challenge, 'node_id': role.node_public_key,
                    'network_id': self._inputs.network_id, 'genesis_hash': self._inputs.genesis_hash,
                    'context_id': self._inputs.context_id}
        _require(type(value['version']) is int and value['version'] == 1
                 and all(type(value[name]) is str and value[name] == item
                         for name, item in expected.items()), 'readiness_report_binding')
        if value['state'] == 'pending':
            _require(type(value['reason']) is str and value['reason'] in _PENDING
                     and value['attestation_norito_base64'] is None, 'readiness_pending_invalid')
            return None
        _require(value['state'] == 'ready' and value['reason'] is None, 'readiness_terminal_failure')
        encoded = value['attestation_norito_base64']
        _require(type(encoded) is str and 0 < len(encoded) <= ((MAX_ATTESTATION_BYTES + 2) // 3) * 4,
                 'readiness_attestation_invalid')
        attestation = base64.b64decode(encoded, validate=True)
        _require(0 < len(attestation) <= MAX_ATTESTATION_BYTES
                 and base64.b64encode(attestation).decode('ascii') == encoded,
                 'readiness_attestation_invalid')
        return ReadyReceipt(role.peer_id, identity, role.node_public_key,
            self._inputs.network_id, self._inputs.genesis_hash, self._inputs.context_id,
            challenge, self._image.sha256, cli_identity, role.client_config_sha256, self._inputs.anchors_sha256,
            hashlib.sha256(raw).hexdigest(), attestation)

    def collect(self, peers: tuple[PinnedProcess, ...], verify_peers: Callable[[], None]):
        """Invoke the native verifier for all four original live process roles."""
        try:
            _require(self._phase == 'admitted' and type(peers) is tuple and len(peers) == 4
                     and callable(verify_peers), 'readiness_phase_invalid')
            _require(all(isinstance(peer, PinnedProcess) and peer.peer_id == role.peer_id
                         for peer, role in zip(peers, self._inputs.roles, strict=True)),
                     'readiness_original_peers_invalid')
            self._phase, self._guard = 'busy', verify_peers
            identities = tuple(peer.identity for peer in peers)
            for index in range(4):
                while True:
                    self._verify()
                    challenge = secrets.token_hex(32)
                    _require(type(challenge) is str and len(challenge) == 64
                             and all(ch in '0123456789abcdef' for ch in challenge)
                             and challenge != '0' * 64 and challenge not in self._challenges,
                             'readiness_challenge_invalid')
                    self._challenges.add(challenge)
                    _require(len(self._challenges) <= MAX_TRIAL_NS // RETRY_NS + 4,
                             'readiness_attempt_bound_exceeded')
                    raw, cli_identity = self._invoke(index, challenge)
                    receipt = self._report(index, challenge, raw, identities[index], cli_identity)
                    self._verify()
                    if receipt is not None:
                        self._receipts.append(receipt)
                        break
                    time.sleep(min(RETRY_NS / 1_000_000_000, _remaining(self._end)))
            self._verify()
            _require(len(self._receipts) == 4 and self._phase == 'busy', 'readiness_incomplete')
            self._phase = 'ready'
            return tuple(self._receipts)
        except BaseException as error:
            self._phase = 'failed'
            _public_failure(error)

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Fail permanently and reap original CLI handles under a cleanup cap."""
        self._phase = 'failed'
        return self._commands.cleanup(deadline_ns)
