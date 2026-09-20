"""Collect exact native finality/query vectors from the stopped original peer.

This owner invokes only the current native CLI collector. Native code owns Norito,
cryptography, complete-store scheduling and leaf reconciliation. The mandatory
runtime callback binds the original peer3 clean stop, exact stopped tip and peer0
liveness; this component alone never proves those lifecycle facts or a release.
Keep original ReadinessInputs and NativeOutputs open through later facts/replay.

TODO: retain the original complete Native context-witness archive and forward its
path, raw SHA-256 and byte reservation to collect-scaling-inputs. The canonical
collector requires that archive; genesis-only contexts and finality proofs cannot
replace it. The fixed trial cannot qualify collection until this producer joins
the stopped-peer input owner.
"""
from __future__ import annotations

from dataclasses import dataclass
import fcntl
import json
import os
from pathlib import Path
import re
import secrets
import time
from typing import Callable

from resource_process import ExecutableImage, ProcessIdentity
from scaling_command import BoundedCommand, MAX_TRIAL_NS
from scaling_native_outputs import NativeOutputs, PublishedIdentity, RetainedOutput
from scaling_proof_sequence import StoppedReader, _reader_snapshot
from scaling_readiness_inputs import ReadinessInputs

_MIB = 1024 * 1024
_HEX = re.compile(r'[0-9a-f]{64}')
_FIELDS = frozenset(('version', 'operation', 'invocation_id', 'client_config_sha256',
    'committed_height', 'finality_count', 'query_count', 'context_sha256', 'context_bytes',
    'finality_sha256', 'finality_bytes', 'queries_sha256', 'queries_bytes'))


class VectorCollectionError(ValueError):
    """Closed public code without native stderr, private config or proof contents."""


def _require(condition):
    if not condition: raise VectorCollectionError('vector_collection_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise VectorCollectionError('vector_collection_failed') from None


def _integer(value, minimum, maximum):
    _require(type(value) is int and minimum <= value <= maximum)
    return value


def _digest(value):
    _require(type(value) is str and _HEX.fullmatch(value) is not None)
    return value


@dataclass(frozen=True, slots=True)
class CollectionLimits:
    """Independent query/context/client allocations, admitted before dispatch.

    The canonical StoppedReader carries stopped-store geometry and reader caps.
    NativeOutputs carries the independently admitted finality/query file caps.
    total_max_bytes reserves original client + context + both output file caps.
    """
    client_config_max_bytes: int
    context_max_bytes: int
    total_max_bytes: int
    reply_max_bytes: int
    max_total_leaves: int
    max_leaves_per_carrier: int
    max_decode_bytes: int


def _limits(value):
    _require(type(value) is CollectionLimits)
    result = tuple(getattr(value, key) for key in value.__dataclass_fields__)
    client, context, total, reply, leaves, carrier_leaves, decode = result
    _integer(client, 1, _MIB)
    _integer(context, 1, 8 * _MIB)
    _integer(total, 1, 256 * _MIB)
    _integer(reply, 1, 4096)
    _integer(leaves, 1, 1_000_000)
    _integer(carrier_leaves, 1, leaves)
    _integer(decode, 1, 512 * _MIB)
    return result


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        _require(type(key) is str and key not in result)
        result[key] = value
    return result


def _number(raw):
    _require(len(raw) <= 20)
    return int(raw)


def _invalid(_):
    raise VectorCollectionError('vector_collection_failed')


def _reply(raw, maximum):
    _require(type(raw) is bytes and 1 < len(raw) <= maximum and raw.startswith(b'{')
             and raw.endswith(b'}\n') and b'\n' not in raw[:-1] and raw.isascii())
    value = json.loads(raw, object_pairs_hook=_pairs, parse_int=_number,
                       parse_float=_invalid, parse_constant=_invalid)
    _require(type(value) is dict and value.keys() == _FIELDS)
    return value


@dataclass(frozen=True, slots=True)
class VectorCollectionReceipt:
    """Public native result; the original owners retain all input/output FDs."""
    invocation_id: str
    process: ProcessIdentity
    cli_sha256: str
    anchors_sha256: str
    client_config_sha256: str
    context_sha256: str
    context_bytes: int
    stopped_height: int
    finality_count: int
    query_count: int
    finality: RetainedOutput
    queries: RetainedOutput


class NativeVectorCollection:
    """One original-deadline collection, with retained child cleanup on failure."""

    def __init__(self, inputs: ReadinessInputs, outputs: NativeOutputs, stopped: StoppedReader,
                 limits: CollectionLimits, image: ExecutableImage, reader,
                 trial_deadline_ns: int, verify_original_runtime: Callable[[], None]):
        try:
            _require(not hasattr(self, '_inputs') and type(inputs) is ReadinessInputs
                     and type(outputs) is NativeOutputs and isinstance(image, ExecutableImage)
                     and callable(getattr(reader, 'sample', None)) and callable(verify_original_runtime))
            _integer(trial_deadline_ns, 1, (1 << 64) - 1)
            _require(0 < trial_deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS)
            # Own every admitted scalar before any callback. Later caller dataclass changes
            # cannot retarget native commands or increase these original reservations.
            self._store, self._merge, self._reader_args = _reader_snapshot(stopped)
            self._limits = _limits(limits)
            first, last, blocks, data, carrier, merge_bytes, frames, input_bytes, value_decode, uid = self._reader_args
            _require(first == 1 and uid == os.geteuid())
            _integer(data, 1, 2 * 1024 * _MIB)
            _integer(carrier, 1, 32 * _MIB)
            _integer(merge_bytes, 1, 256 * _MIB)
            _integer(frames, 1, blocks)
            _integer(input_bytes, 1, 256 * _MIB)
            _integer(value_decode, 1, self._limits[6])
            inputs.validate(); outputs.validate(); image.validate()
            self._inputs, self._outputs, self._image = inputs, outputs, image
            self._end, self._original_end = trial_deadline_ns, trial_deadline_ns
            self._guard = verify_original_runtime
            self._image_binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
            self._input_binding = self._snapshot_inputs()
            self._output_binding = (outputs.directory, outputs.path('finality'), outputs.path('queries'),
                                    outputs.allocation('finality'), outputs.allocation('queries'))
            _require(self._store == str(inputs.roles[3].primary_block_store)
                     and self._merge == str(inputs.roles[3].primary_merge_log))
            _require(outputs.directory != inputs.input_directory
                     and inputs.input_directory not in outputs.directory.parents
                     and outputs.directory not in inputs.input_directory.parents)
            _require(image.path != outputs.directory and outputs.directory not in image.path.parents)
            artifacts = {item.path: item for item in inputs.generation.artifacts}
            context = artifacts['genesis-context.nrt']
            client = artifacts[inputs.roles[0].client_config.name]
            _require(client.sha256 == inputs.roles[0].client_config_sha256)
            self._context = (inputs.input_directory / context.path, _digest(context.sha256),
                             _integer(context.bytes, 1, self._limits[1]))
            self._client = (inputs.roles[0].client_config, _digest(client.sha256),
                            _integer(client.bytes, 1, self._limits[0]), inputs.client_fd(0))
            _require(self._limits[0] + self._limits[1] + sum(self._output_binding[3:]) <= self._limits[2])
            self._phase, self._receipt = 'admitted', None
            self._commands = BoundedCommand(image, reader, trial_deadline_ns, self._verify)
        except BaseException as error:
            _failure(error)

    def _snapshot_inputs(self):
        inputs = self._inputs
        inputs.validate()
        return (inputs.input_directory, inputs.anchors_sha256, inputs.genesis_hash,
                inputs.context_id, inputs.network_id,
                tuple(tuple(getattr(role, field) for field in role.__dataclass_fields__) for role in inputs.roles),
                tuple((item.path, item.sha256, item.bytes) for item in inputs.generation.artifacts))

    def _check(self):
        _require(self._phase in ('collecting', 'collected') and self._end == self._original_end
                 and self._commands.deadline_ns == self._original_end
                 and time.monotonic_ns() < self._original_end)
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._image_binding)
        image.validate()
        _require(self._snapshot_inputs() == self._input_binding)
        self._outputs.validate()
        _require((self._outputs.directory, self._outputs.path('finality'), self._outputs.path('queries'),
                  self._outputs.allocation('finality'), self._outputs.allocation('queries')) == self._output_binding)
        _require(self._inputs.client_fd(0) == self._client[3]
                 and fcntl.fcntl(self._client[3], fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY)

    def _verify(self):
        self._check()
        self._guard()
        self._check()

    def _argv(self, invocation):
        _, last, blocks, data, carrier, merge_bytes, frames, input_bytes, value_decode, _ = self._reader_args
        client_cap, context_cap, total, reply, leaves, carrier_leaves, decode = self._limits
        context, context_sha, _ = self._context
        client, client_sha, _, fd = self._client
        _, finality, queries, finality_cap, queries_cap = self._output_binding
        fields = (('invocation-id', invocation), ('network-id', self._input_binding[4]),
            ('client-config-sha256', client_sha), ('client-config-max-bytes', client_cap),
            ('deadline-monotonic-ns', self._original_end), ('block-store', self._store), ('merge-log', self._merge),
            ('context', context), ('context-sha256', context_sha), ('context-max-bytes', context_cap),
            ('finality-out', finality), ('queries-out', queries), ('finality-max-bytes', finality_cap),
            ('queries-max-bytes', queries_cap), ('total-max-bytes', total), ('reply-max-bytes', reply),
            ('last-height', last), ('max-committed-blocks', blocks), ('max-store-data-bytes', data),
            ('max-carrier-bytes', carrier), ('max-merge-log-bytes', merge_bytes), ('max-merge-frames', frames),
            ('max-input-bytes', input_bytes), ('max-total-leaves', leaves),
            ('max-leaves-per-carrier', carrier_leaves), ('max-decode-bytes', decode),
            ('max-value-decode-bytes', value_decode))
        return (str(self._image_binding[0]), '--machine', '--config-fd', str(fd),
            '--config-source-path', str(client), '--output-format', 'json', 'tx', 'collect-scaling-inputs',
            *(part for flag, value in fields for part in ('--' + flag, str(value))))

    def run(self) -> VectorCollectionReceipt:
        """Return only after native exit zero, exact reply and complete pair custody."""
        try:
            _require(self._phase == 'admitted')
            self._phase = 'collecting'
            self._verify()
            invocation = _digest(secrets.token_hex(32))
            _require(invocation != '0' * 64)
            argv = self._argv(invocation)
            self._outputs.begin('collection')
            result = self._commands.run('vector-collection', argv, (self._client[3],), self._limits[3])
            self._verify()
            value = _reply(result.stdout, self._limits[3])
            _require(type(value['version']) is int and value['version'] == 1
                     and value['operation'] == 'collect_scaling_inputs' and value['invocation_id'] == invocation
                     and value['client_config_sha256'] == self._client[1]
                     and value['context_sha256'] == self._context[1]
                     and type(value['context_bytes']) is int and value['context_bytes'] == self._context[2])
            last = self._reader_args[1]
            _require(type(value['committed_height']) is int and value['committed_height'] == last
                     and type(value['finality_count']) is int and value['finality_count'] == last)
            query_count = _integer(value['query_count'], 0, self._limits[4])
            replies = tuple(PublishedIdentity(role, _digest(value[role + '_sha256']),
                _integer(value[role + '_bytes'], 1, self._output_binding[3 + index]))
                for index, role in enumerate(('finality', 'queries')))
            self._verify()
            finality, queries = self._outputs.complete(replies)
            self._verify()
            receipt = VectorCollectionReceipt(invocation, result.process, self._image_binding[3],
                self._input_binding[1], self._client[1], self._context[1], self._context[2], last,
                last, query_count, finality, queries)
            self._receipt, self._phase = receipt, 'collected'
            return receipt
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def validate(self):
        """Recheck this collected pair and original caller-owned lifetime."""
        try:
            _require(self._phase == 'collected' and self._receipt is not None)
            self._verify()
            _require(self._outputs.artifact('finality') == self._receipt.finality
                     and self._outputs.artifact('queries') == self._receipt.queries)
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Reap retained native child handles; never close borrowed originals."""
        self._phase = 'failed'
        return self._commands.cleanup(deadline_ns)
