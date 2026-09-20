"""Join retained resource replay to the original fixed native workload plan.

Requires the completed ResourceReplay result, original retained ReadinessInputs,
original FactsJournalPlan and original stopped-tip StoppedReader. This adapter
does no journal parsing, Norito decoding, process work or proof authentication.
The fixed runner retains those owners and authenticates their provenance through
canonical native facts/export/replay before accepting any experiment result.
"""
from dataclasses import fields
import hashlib
import os

from applied_request_journal import RetainedApplication
from resource_evidence_budget import (
    MAX_CONTROL_FILES, MAX_TOTAL_BYTES, validate_run_budget,
)
from resource_replay import Bracket, CaptureReduction, ReplayGeometry, ReplayResult
from scaling_canonical_proof import AppliedObservation, ReplayPlan, _digest, _integer, _plan_snapshot
from scaling_native_facts_inputs import FactsJournalPlan, I64, U128, journal_snapshot
from scaling_proof_sequence import StoppedReader, _reader_snapshot
from scaling_readiness_inputs import ReadinessInputs
from signed_request_journal import MAX_REQUEST_BYTES, RequestPlan, RetainedRequest


class ReplayedWorkloadError(ValueError):
    """Closed adapter failure without private input contents or native stderr."""


def _require(value, code):
    if not value:
        raise ReplayedWorkloadError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    if type(error) is ReplayedWorkloadError:
        raise ReplayedWorkloadError(str(error)) from None
    raise ReplayedWorkloadError('replayed_workload_invalid') from None


def _original(inputs):
    _require(type(inputs) is ReadinessInputs, 'replayed_workload_inputs_required')
    inputs.validate()
    generation, roles = inputs.generation, inputs.roles
    _require(type(roles) is tuple and len(roles) == 4
             and tuple(role.peer_id for role in roles) == ('peer0', 'peer1', 'peer2', 'peer3'),
             'replayed_workload_peer_scope')
    _require(type(generation.lane_count) is int and generation.lane_count in (1, 4),
             'replayed_workload_lane_count')
    return (inputs.anchors_sha256, inputs.network_id, generation.lane_count,
            tuple(account.account_id for account in generation.accounts),
            roles[3].primary_block_store, roles[3].primary_merge_log)


def _resource_scope(replayed, selected, lanes):
    """Compare replay's retained original scope, not a reconstructed journal plan."""
    _require(type(replayed) is ReplayResult, 'replayed_workload_result_required')
    geometry = replayed.geometry
    _require(type(geometry) is ReplayGeometry, 'replayed_workload_geometry_required')
    geometry.validate()
    actual = tuple(getattr(geometry, field.name) for field in fields(ReplayGeometry))
    expected = tuple(selected[index] for index in (4, 5, 6, 10, 16, 17, 18))
    _require(actual == expected and all(type(value) is int for value in actual),
             'replayed_workload_original_geometry')
    allocation = validate_run_budget(replayed.allocation)
    _require(allocation.run.pair_index == selected[1]
             and allocation.run.variant == ('one_lane' if lanes == 1 else 'four_lane')
             and (allocation.geometry.peers, allocation.geometry.interval_ns,
                  allocation.geometry.measurement_ns, allocation.geometry.drain_ns)
             == (4, selected[16], selected[5], selected[6]),
             'replayed_workload_original_allocation')
    reservations = (
        ('admitted_resource_byte_limit', allocation.resource_bytes),
        ('admitted_journal_byte_limit', allocation.journal.max_bytes),
        ('admitted_resource_file_count', allocation.member_count),
        ('admitted_experiment_resource_bytes', allocation.experiment.resource_bytes),
        ('admitted_experiment_resource_file_count', allocation.experiment.resource_member_count),
        ('admitted_experiment_total_bytes', allocation.experiment.total_bytes),
        ('global_byte_limit', MAX_TOTAL_BYTES), ('control_file_limit', MAX_CONTROL_FILES),
        ('capture_file_count', allocation.member_count),
    )
    for name, value in reservations:
        _require(type(getattr(replayed, name)) is int and getattr(replayed, name) == value,
                 'replayed_workload_reservation_changed')
    _digest(replayed.journal_sha256)
    journal_bytes = _integer(replayed.journal_bytes, 1, allocation.journal.max_bytes)
    total = _integer(replayed.capture_bytes, 1, allocation.resource_bytes)
    _require(_integer(replayed.raw_body_bytes, 1, total)
             + _integer(replayed.manifest_bytes, 1, total) == total,
             'replayed_workload_resource_bytes')
    samples = replayed.samples
    _require(type(samples) is tuple and len(samples) == geometry.samples
             and type(replayed.preflight) is CaptureReduction
             and type(replayed.preflight.sequence) is int and replayed.preflight.sequence == 0,
             'replayed_workload_sample_coverage')
    previous_end = 0
    for index, sample in enumerate(samples):
        _require(type(sample) is Bracket and type(sample.capture) is CaptureReduction,
                 'replayed_workload_sample_type')
        scheduled = index * selected[16]
        _require(type(sample.scheduled_offset_ns) is int and sample.scheduled_offset_ns == scheduled
                 and type(sample.capture.sequence) is int and sample.capture.sequence == index + 1,
                 'replayed_workload_sample_schedule')
        start = _integer(sample.start_offset_ns, max(scheduled, previous_end), scheduled + selected[18])
        deadline = min(start + selected[17], geometry.final + selected[17])
        previous_end = _integer(sample.end_offset_ns, start, deadline - 1)
    finish_start = _integer(replayed.finish_start_ns, previous_end, I64)
    _integer(replayed.finish_end_ns, finish_start, min(I64, finish_start + selected[17] - 1))
    return journal_bytes


def build_replay_plan(replayed: ReplayResult, inputs: ReadinessInputs,
                      plan: FactsJournalPlan, stopped: StoppedReader) -> ReplayPlan:
    """Return the exact native proof plan from completed retained replay only.

    CLI transaction_load.rs::Schedule and its workload.rs::account_offset
    define the native schedule. Kagami's export/launcher/journal/schedule.rs
    independently checks it: cohort ordinals restart, warmup starts at -(W+D),
    rational offsets floor n*1e9*denominator/numerator, and account rotation
    uses the first eight little-endian bytes of its SHA-256.
    No returned projection authenticates signed bodies or canonical execution.
    """
    try:
        original = _original(inputs)
        selected, counts = journal_snapshot(plan, original[3])
        store, merge, reader = _reader_snapshot(stopped)
        _require(store == str(original[4]) and merge == str(original[5])
                 and reader[-1] == os.geteuid(), 'replayed_workload_stopped_geometry')
        journal_bytes = _resource_scope(replayed, selected, original[2])
        signed, applications = replayed.signed_requests, replayed.applied_requests
        total = sum(counts)
        _require(type(signed) is tuple and type(applications) is tuple
                 and len(signed) == len(applications) == total,
                 'replayed_workload_request_coverage')
        seed, _, numerator, denominator, warmup, measurement, drain, lag = selected[:8]
        period = 1_000_000_000 * denominator
        _require((max(counts) - 1) * period <= U128, 'replayed_workload_native_offset_overflow')
        rotation = int.from_bytes(hashlib.sha256(
            f'gscale-account-offset-v1:{seed}'.encode('ascii')).digest()[:8], 'little') % len(original[3])
        observations, seen, canonical_bytes = [], set(), 0
        for index, (request, application) in enumerate(zip(signed, applications, strict=True)):
            _require(type(request) is RetainedRequest and type(request.plan) is RequestPlan
                     and type(application) is RetainedApplication,
                     'replayed_workload_request_type')
            _require(type(request.index) is int and type(application.index) is int
                     and request.index == application.index == index,
                     'replayed_workload_request_index')
            tx_hash = _digest(request.hash, marked=True)
            _require(type(application.hash) is str and application.hash == tx_hash and tx_hash not in seen,
                     'replayed_workload_request_hash')
            seen.add(tx_hash)
            _digest(request.canonical_sha256)
            _require(type(request.canonical_bytes) is bytes
                     and 0 < len(request.canonical_bytes) <= MAX_REQUEST_BYTES,
                     'replayed_workload_signed_bytes')
            canonical_bytes += len(request.canonical_bytes)
            _require(canonical_bytes <= journal_bytes // 2, 'replayed_workload_signed_allocation')
            cohort, ordinal, start = ('warmup', index, -(warmup + drain)) if index < counts[0] else (
                'measurement', index - counts[0], 0)
            scheduled = start + ordinal * period // numerator
            expected = (cohort, ordinal + 1, hashlib.sha256(
                f'{seed}:{cohort}:{ordinal + 1}'.encode('ascii')).hexdigest(), scheduled,
                (ordinal + rotation) % len(original[3]))
            actual = tuple(getattr(request.plan, field.name) for field in fields(RequestPlan))
            _require(all(type(left) is type(right) and left == right
                         for left, right in zip(actual, expected, strict=True)),
                     'replayed_workload_original_schedule')
            phase_end = -drain if cohort == 'warmup' else measurement
            offer = _integer(application.offer_offset_ns, scheduled, min(scheduled + lag, phase_end - 1))
            final = -1 if cohort == 'warmup' else measurement + drain
            _integer(application.acknowledgment_offset_ns, offer, final)
            _integer(application.applied_offset_ns, offer + 1, final)
            _integer(application.local_applied_offset_ns, offer + 1, final)
            height = _integer(application.block_height, 1, reader[1])
            _require(_integer(application.local_block_height, 1, reader[1]) == height,
                     'replayed_workload_applied_height')
            _integer(application.status_attempts, 1)
            _integer(application.local_status_attempts, 1)
            observations.append(AppliedObservation(tx_hash, height, height))
        result = ReplayPlan(seed, original[2], tuple(original[3]), counts[0], counts[1],
                            reader[1], tuple(observations))
        _plan_snapshot(result)
        _require(_original(inputs) == original, 'replayed_workload_original_changed')
        return result
    except BaseException as error:
        _failure(error)
