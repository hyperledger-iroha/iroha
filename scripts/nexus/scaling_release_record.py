"""Canonical data projection of the original release parent's observations.

Decoding this record does not authenticate execution. Local acceptance belongs
to the original parent holding its command/input owners. A detached consumer
also needs an externally authenticated final parent marker digest; a digest
provided beside this record is only a comparison input. No archived PID, FD,
pathname or monotonic deadline is adopted as a current execution authority.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
import re
import stat

from resource_evidence_budget import canonical_run_budget_bytes, select_run_budget
from resource_replay import _json
from scaling_archive_data import ArchiveBindings, ArchiveInventory, inspect_archive, _Tree, _layout
from scaling_experiment_config import MAX_CONFIG_BYTES, decode_fixed_inputs
from scaling_cli_bootstrap import MAX_PARENT_EXECUTION_BYTES, validate_dependency_binding
from scaling_experiment_plan import RUN_KEYS, encode
from scaling_preflight_archive import validate_binding as validate_preflight_binding
import scaling_preflight_archive as preflight_archive

SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.parent_execution.v1'
ARCHIVE_ID = 'release-scaling.fixed-collector.v1'
RECORD_ARCHIVE_ID = 'release-scaling.parent-execution.v1'
MAX_RECORD_BYTES = MAX_PARENT_EXECUTION_BYTES
COLLECTOR_OPERATION = 'multilane-scaling.fixed-collector.v1'
NATIVE_OPERATION = 'multilane-scaling.canonical-native-replay.v1'
_NS = 1_000_000_000


class ScalingReleaseRecordError(ValueError):
    """Closed record failure without private or untrusted evidence contents."""


def _require(value):
    if not value:
        raise ScalingReleaseRecordError('scaling_release_record_invalid')


def _fields(value, names):
    _require(type(value) is dict and set(value) == set(names))
    return value


def _integer(value, minimum=0, maximum=(1 << 63) - 1):
    _require(type(value) is int and minimum <= value <= maximum)
    return value


def _digest(value):
    _require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None)
    return value


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _stream(value, maximum):
    _fields(value, ('bytes', 'sha256'))
    _integer(value['bytes'], 0, maximum)
    _digest(value['sha256'])
    if value['bytes'] == 0:
        _require(value['sha256'] == _sha(b''))


def _command(value, operation, earliest, deadline, maximum):
    _fields(value, ('operation_id', 'argv_sha256', 'environment_sha256',
        'started_ns', 'completed_ns', 'returncode', 'stdout', 'stderr', 'violations'))
    _require(value['operation_id'] == operation)
    _digest(value['argv_sha256']); _digest(value['environment_sha256'])
    start = _integer(value['started_ns'], earliest, deadline)
    end = _integer(value['completed_ns'], start, deadline)
    _require(type(value['returncode']) is int and value['returncode'] == 0
             and type(value['violations']) is list and value['violations'] == [])
    _stream(value['stdout'], maximum); _stream(value['stderr'], maximum)
    _require(value['stdout']['bytes'] + value['stderr']['bytes'] <= maximum)
    return end


@dataclass(frozen=True, slots=True)
class ParentExecutionRecord:
    """Owned canonical data and admitted policy; deliberately no authority bit."""
    canonical: bytes
    sha256: str
    plan: object
    budget: object
    bindings: ArchiveBindings
    inventory: ArchiveInventory

    @property
    def document(self):
        """Return a fresh data projection, never mutable retained parser state."""
        return json.loads(self.canonical)


def decode_parent_execution(raw: bytes) -> ParentExecutionRecord:
    """Decode one bounded exact schema, using the existing plan/budget owner."""
    try:
        value = _json(raw, MAX_RECORD_BYTES)
        _require(encode(value, MAX_RECORD_BYTES) == raw)
        _fields(value, ('schema', 'candidate', 'inputs', 'execution', 'publication', 'verifier_python', 'preflight'))
        validate_dependency_binding(value['verifier_python'])
        preflight = validate_preflight_binding(value['preflight'])
        _require(value['schema'] == SCHEMA)
        candidate = _fields(value['candidate'],
            ('head_commit', 'head_tree', 'workspace_source_manifest_sha256'))
        lengths = set()
        for name in ('head_commit', 'head_tree'):
            item = candidate[name]
            _require(type(item) is str and re.fullmatch('(?:[0-9a-f]{40}|[0-9a-f]{64})', item))
            lengths.add(len(item))
        _require(len(lengths) == 1)
        _digest(candidate['workspace_source_manifest_sha256'])
        inputs = _fields(value['inputs'],
            ('plan', 'budget', 'identity_sha256', 'source_closure_sha256'))
        plan, budget, raw_plan = decode_fixed_inputs(
            encode(inputs['plan'], MAX_CONFIG_BYTES), encode(inputs['budget'], MAX_CONFIG_BYTES))
        _require(encode(inputs['plan'], MAX_CONFIG_BYTES) == raw_plan
                 and encode(inputs['budget'], MAX_CONFIG_BYTES)
                 == canonical_run_budget_bytes(select_run_budget(budget, 1, 'one_lane')))
        _digest(inputs['identity_sha256']); _digest(inputs['source_closure_sha256'])
        execution = _fields(value['execution'], ('invocation_sha256', 'launch_input_sha256',
            'original_started_ns', 'deadline_ns', 'observation_overhead_seconds',
            'kagami_sha256', 'command', 'native_replays', 'verification_completed_ns'))
        for name in ('invocation_sha256', 'launch_input_sha256', 'kagami_sha256'):
            _digest(execution[name])
        start = _integer(execution['original_started_ns'], 1)
        _require(preflight['scope']['completed_ns'] <= start)
        overhead = _integer(execution['observation_overhead_seconds'], 0, 600)
        deadline = _integer(execution['deadline_ns'], start + 1)
        _require(deadline - start == ((plan.experiment_timeout_ns + _NS - 1) // _NS + overhead) * _NS)
        previous = _command(execution['command'], COLLECTOR_OPERATION, start, deadline,
                            sum(row.max_bytes for row in budget.control_budgets[:2]))
        native = execution['native_replays']
        _require(type(native) is list and len(native) == len(RUN_KEYS))
        invocations = set()
        for expected, trial, row in zip(RUN_KEYS, plan.trials, native, strict=True):
            _fields(row, ('pair_index', 'variant', 'invocation_id', 'request_sha256',
                'proof_sha256', 'proof_iroha_hash', 'proof_bytes', 'row_count',
                'reply_bytes', 'reply_sha256', 'joined_rows_sha256', 'command'))
            _require(type(row['pair_index']) is int and type(row['variant']) is str
                     and (row['pair_index'], row['variant']) == expected)
            for name in ('invocation_id', 'request_sha256', 'proof_sha256',
                         'proof_iroha_hash', 'reply_sha256', 'joined_rows_sha256'):
                _digest(row[name])
            _require(row['invocation_id'] != '0' * 64 and row['invocation_id'] not in invocations)
            invocations.add(row['invocation_id'])
            allocation = select_run_budget(budget, *expected)
            _integer(row['proof_bytes'], 1, allocation.run.canonical_proof.max_bytes)
            _integer(row['row_count'], 1, 1_000_000)
            _integer(row['reply_bytes'], 1, trial.replay_reply_max_bytes)
            previous = _command(row['command'], NATIVE_OPERATION, previous, deadline,
                                trial.replay_reply_max_bytes)
            _require(row['command']['environment_sha256'] == execution['command']['environment_sha256']
                     and row['reply_bytes'] == row['command']['stdout']['bytes']
                     and row['reply_sha256'] == row['command']['stdout']['sha256'])
        _integer(execution['verification_completed_ns'], previous, deadline)
        publication = _fields(value['publication'], ('manifest_sha256', 'report_sha256', 'inventory'))
        inventory = _fields(publication['inventory'], ('files', 'bytes', 'sha256'))
        _require(_integer(inventory['files'], 1) == budget.control_file_count + budget.resource_member_count)
        _integer(inventory['bytes'], 1, budget.total_bytes)
        binding = ArchiveBindings(_digest(publication['manifest_sha256']),
            _digest(publication['report_sha256']), _digest(inventory['sha256']))
        return ParentExecutionRecord(raw, _sha(raw), plan, budget, binding,
            ArchiveInventory(inventory['files'], inventory['bytes'], inventory['sha256']))
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None


def _project_command(command, operation, deadline):
    _require(command.deadline_ns == deadline and type(command.argv) is tuple
             and all(type(item) is str for item in command.argv)
             and command.violations == ())
    return dict(operation_id=operation, argv_sha256=_sha(encode(list(command.argv))),
        environment_sha256=command.environment_sha256, started_ns=command.started_ns,
        completed_ns=command.completed_ns, returncode=command.returncode,
        stdout=dict(bytes=command.stdout_bytes, sha256=command.stdout_sha256),
        stderr=dict(bytes=command.stderr_bytes, sha256=command.stderr_sha256), violations=[])


def encode_parent_execution(candidate_identity, verification_inputs, original_observation,
                            verification, *, observation_overhead_seconds, verifier_python,
                            preflight) -> bytes:
    """Project the parent's retained objects; callers retain their actual custody.

    This serializer is not a constructor for acceptance. The original operation
    owns all object identities; final bootstrap publication compares these exact
    bytes again against that same operation and its still-retained artifacts.
    """
    try:
        plan, budget, canonical_plan = decode_fixed_inputs(verification_inputs.plan_bytes,
                                                        verification_inputs.budget_bytes)
        checked = verification.archive_checks
        _require(verification.collector is original_observation.command
            and verification.invocation_sha256 == original_observation.invocation_sha256
            and verification.launch_input_sha256 == original_observation.launch_input_sha256
            and verification.artifacts == original_observation.artifacts
            and verification.plan_sha256 == _sha(verification_inputs.plan_bytes)
            and verification.budget_sha256 == _sha(verification_inputs.budget_bytes)
            and verification.kagami_sha256 == verification_inputs.kagami_sha256
            and verification.inventory == checked.inventory
            and candidate_identity['head_commit'] == verification_inputs.source_revision
            and candidate_identity['workspace_source_manifest_sha256'] == verification_inputs.workspace_source_sha256)
        artifacts = {row.relative_path: row for row in verification.artifacts}
        _require(set(artifacts) == {'manifest.json', 'report.json'}
            and artifacts['manifest.json'].sha256 == checked.bindings.manifest_sha256
            and artifacts['report.json'].sha256 == checked.bindings.report_sha256
            and checked.inventory.sha256 == checked.bindings.inventory_sha256)
        _require(all(getattr(checked.measurements, name) is True for name in
            ('throughput_criterion_met', 'latency_criterion_met', 'observed_resource_criterion_met')))
        deadline = verification.original_deadline_ns
        native = []
        for row in verification.native_replays:
            data = {name: getattr(row, name) for name in ('pair_index', 'variant', 'invocation_id',
                'request_sha256', 'proof_sha256', 'proof_iroha_hash', 'proof_bytes',
                'row_count', 'reply_bytes', 'reply_sha256', 'joined_rows_sha256')}
            data['command'] = _project_command(row.command, NATIVE_OPERATION, deadline)
            native.append(data)
        value = dict(schema=SCHEMA,
            verifier_python=validate_dependency_binding(verifier_python),
            preflight=validate_preflight_binding(preflight),
            candidate={name: candidate_identity[name] for name in
                ('head_commit', 'head_tree', 'workspace_source_manifest_sha256')},
            inputs=dict(plan=json.loads(canonical_plan),
                budget=json.loads(canonical_run_budget_bytes(select_run_budget(budget, 1, 'one_lane'))),
                identity_sha256=_sha(checked.identity_json), source_closure_sha256=_sha(checked.source_closure_json)),
            execution=dict(invocation_sha256=verification.invocation_sha256,
                launch_input_sha256=verification.launch_input_sha256,
                original_started_ns=original_observation.original_started_ns, deadline_ns=deadline,
                observation_overhead_seconds=observation_overhead_seconds,
                kagami_sha256=verification.kagami_sha256,
                command=_project_command(verification.collector, COLLECTOR_OPERATION, deadline),
                native_replays=native, verification_completed_ns=verification.verification_completed_ns),
            publication=dict(manifest_sha256=checked.bindings.manifest_sha256,
                report_sha256=checked.bindings.report_sha256,
                inventory=dict(files=checked.inventory.files, bytes=checked.inventory.bytes,
                               sha256=checked.inventory.sha256)))
        return decode_parent_execution(encode(value, MAX_RECORD_BYTES)).canonical
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None


def inspect_parent_archive(root, record, candidate_identity):
    """Recompute archive data and recorded-row joins; native crypto is separate."""
    try:
        _require(type(record) is ParentExecutionRecord)
        record = decode_parent_execution(record.canonical)
        value = record.document
        _require(value['candidate'] == {name: candidate_identity[name] for name in value['candidate']})
        checked = inspect_archive(root, record.plan, record.budget, record.bindings)
        _require(checked.inventory == record.inventory
            and _sha(checked.identity_json) == value['inputs']['identity_sha256']
            and _sha(checked.source_closure_json) == value['inputs']['source_closure_sha256'])
        identity, source = json.loads(checked.identity_json), json.loads(checked.source_closure_json)
        _require(identity['software']['source_revision'] == value['candidate']['head_commit']
            and identity['software']['workspace_source_sha256'] == value['candidate']['workspace_source_manifest_sha256']
            and source['executable_images']['kagami'] == value['execution']['kagami_sha256'])
        _require(all(getattr(checked.measurements, name) is True for name in
            ('throughput_criterion_met', 'latency_criterion_met', 'observed_resource_criterion_met')))
        for run, observed in zip(checked.native_runs, value['execution']['native_replays'], strict=True):
            files = {row.role: row for row in run.files}
            canonical = json.loads(run.canonical_receipt_json)
            _require((run.pair_index, run.variant) == (observed['pair_index'], observed['variant'])
                and observed['request_sha256'] == files['native_request'].binding.sha256
                and observed['proof_sha256'] == files['canonical_proof'].binding.sha256
                and observed['proof_bytes'] == files['canonical_proof'].bytes
                and observed['proof_iroha_hash'] == canonical['proof_iroha_hash']
                and observed['row_count'] == len(run.measurement.requests))
        return checked
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None


def receipt_projection(record):
    """Compact data binding for the aggregate receipt; no authority claim."""
    _require(type(record) is ParentExecutionRecord)
    record = decode_parent_execution(record.canonical)
    return dict(archive_id=ARCHIVE_ID,
        parent_execution=dict(archive_id=RECORD_ARCHIVE_ID, sha256=record.sha256,
                              size_bytes=len(record.canonical), mode='0400'),
        preflight=record.document['preflight'],
        publication=record.document['publication'])


def inspect_preflight_archive(root, record, *, source_root, candidate_identity,
                              invocation_sha256, timeout_seconds):
    """Join retained preflight data to the authenticated source and runner.

    Original process custody remains with the running parent. These historical
    records are checked as data and cannot reconstruct its command owners.
    """
    try:
        _require(type(record) is ParentExecutionRecord)
        record = decode_parent_execution(record.canonical)
        value = record.document
        _require(value['candidate'] == {name: candidate_identity[name]
                                       for name in value['candidate']}
                 and value['execution']['invocation_sha256'] == invocation_sha256)
        return preflight_archive.inspect_preflight_archive(root, value['preflight'],
            source_root=source_root, candidate_identity=value['candidate'],
            invocation_sha256=invocation_sha256,
            collector_started_ns=value['execution']['original_started_ns'],
            timeout_seconds=timeout_seconds)
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None


def capture_preflight_archive(root, record, *, source_root, candidate_identity,
                              invocation_sha256, timeout_seconds):
    """Capture the exact committed subtree for enclosing publication fences."""
    try:
        _require(type(record) is ParentExecutionRecord)
        record = decode_parent_execution(record.canonical)
        value = record.document
        _require(value['candidate'] == {name: candidate_identity[name]
                                       for name in value['candidate']}
                 and value['execution']['invocation_sha256'] == invocation_sha256)
        return preflight_archive.capture_preflight_archive(root, value['preflight'],
            source_root=source_root, candidate_identity=value['candidate'],
            invocation_sha256=invocation_sha256,
            collector_started_ns=value['execution']['original_started_ns'],
            timeout_seconds=timeout_seconds)
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None


def verify_final_execution_join(final_marker: bytes, externally_authenticated_sha256: str,
                                record: ParentExecutionRecord, terminal_receipt_sha256: str) -> None:
    """Join data to an independently authenticated final parent publication.

    Cryptographic archive replay and full aggregate receipt validation remain
    their existing owners' duties. This joins their retained byte commitments;
    archived timing fields are never consulted as a current deadline.
    """
    from scaling_cli_bootstrap import final_execution_record_digest, MAX_FINAL_MARKER_BYTES
    try:
        _require(type(record) is ParentExecutionRecord)
        digest = final_execution_record_digest(final_marker, externally_authenticated_sha256)
        marker = _json(final_marker, MAX_FINAL_MARKER_BYTES)
        value = record.document
        _require(digest == record.sha256
            and marker['scaling_execution'] == receipt_projection(record)
            and marker['candidate_commit_oid'] == value['candidate']['head_commit']
            and marker['candidate_tree_oid'] == value['candidate']['head_tree']
            and marker['retained_source']['source_manifest_sha256']
                == value['candidate']['workspace_source_manifest_sha256']
            and type(marker['receipt_validator']['exit_status']) is int
            and marker['receipt_validator']['exit_status'] == 0
            and marker['terminal_receipt']['sha256'] == _digest(terminal_receipt_sha256))
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None


def capture_public_archive(root, record):
    """Snapshot the exact existing archive owner's census for enclosing fsync.

    Returned modes describe the current copy and are not historical bindings.
    The enclosing receipt owner reopens and retains its own file snapshots and
    repeats this census before publication. The budget owns the entire layout.
    """
    tree = None
    try:
        _require(type(record) is ParentExecutionRecord)
        record = decode_parent_execution(record.canonical)
        tree = _Tree(root, _layout(record.budget), record.budget.total_bytes)
        _require(tree.scan() == record.inventory)
        rows = []
        for name, (size, digest, _) in sorted(tree.rows.items()):
            parent, _, leaf = name.rpartition('/')
            metadata = os.stat(leaf, dir_fd=tree.directories[parent], follow_symlinks=False)
            rows.append(dict(relative_path=name, size_bytes=size, sha256=digest,
                             mode=f'{stat.S_IMODE(metadata.st_mode):04o}', max_bytes=tree.caps[name]))
        rows = tuple(rows)
        directories = tuple(sorted(name for name in tree.directories if name))
        tree.check_metadata()
        return rows, directories
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None
    finally:
        if tree is not None:
            tree.close()


def capture_verifier_archive(root, record):
    """Capture portable verifier files for the enclosing receipt's own fences."""
    from pathlib import Path
    from scaling_cli_bootstrap import (dependency_package_census, validate_dependency_binding,
        bundle_contract, artifact_contract, _MAX_INVENTORY, _MAX_PACKAGE_FILE, _identity)
    held, descriptor = [], None
    try:
        _require(type(record) is ParentExecutionRecord and type(root) is type(Path('/')))
        binding = validate_dependency_binding(record.document['verifier_python'])
        descriptor, _, original = artifact_contract._open_absolute_directory(root, 'retained verifier archive')
        _require(set(os.listdir(descriptor)) == {'source', 'bundle', 'inventory.json'}
                 and original.st_uid == os.geteuid() and stat.S_IMODE(original.st_mode) == 0o700)
        specifications = [('inventory.json', binding['inventory_sha256'], None, _MAX_INVENTORY)]
        for prefix in ('source', 'bundle'):
            _require(dependency_package_census(root / prefix) == tuple(binding['files']))
            specifications.extend((prefix + '/' + row['path'], row['sha256'], row['size_bytes'], _MAX_PACKAGE_FILE)
                                  for row in binding['files'])
        rows, directories = [], {root}
        for relative, digest, size, cap in specifications:
            path = root / relative
            item = bundle_contract._hold_regular(path, 'retained verifier member', maximum_bytes=cap)
            held.append(item)
            metadata, raw = item['metadata'], item['data']
            _require(_sha(raw) == digest and (size is None or len(raw) == size)
                     and metadata.st_uid == os.geteuid()
                     and stat.S_IMODE(metadata.st_mode) in (0o400, 0o600))
            rows.append(dict(relative_path=relative, size_bytes=len(raw), sha256=digest,
                             mode=f'{stat.S_IMODE(metadata.st_mode):04o}', max_bytes=cap))
            directories.update(parent for parent in path.parents if parent == root or root in parent.parents)
        directory_rows = []
        for path in sorted(directories):
            info = path.lstat()
            _require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid()
                     and stat.S_IMODE(info.st_mode) in (0o500, 0o700))
            directory_rows.append(dict(relative_path='' if path == root else str(path.relative_to(root)),
                                       mode=f'{stat.S_IMODE(info.st_mode):04o}'))
        for item in held:
            bundle_contract._revalidate_held_regular(item)
        _require(set(os.listdir(descriptor)) == {'source', 'bundle', 'inventory.json'}
                 and _identity(os.fstat(descriptor)) == _identity(original)
                 and _identity(root.lstat()) == _identity(original))
        return tuple(sorted(rows, key=lambda row: row['relative_path'])), tuple(directory_rows)
    except Exception:
        raise ScalingReleaseRecordError('scaling_release_record_invalid') from None
    finally:
        for item in held:
            from scaling_cli_bootstrap import _close_descriptor
            _close_descriptor(item['descriptor'], item['descriptor_pin'])
            _close_descriptor(item['parent_fd'], item['parent_pin'])
        if descriptor is not None:
            os.close(descriptor)
