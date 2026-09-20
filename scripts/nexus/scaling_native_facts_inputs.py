"""Typed independent allocations for fixed native stopped-tip and facts commands.

Native Rust retains Norito decoding, signatures, state proofs and canonical journal
authority. These snapshots admit finite, explicit command geometry before any child
or output and never populate expectations from a native result or journal plan.
"""
from dataclasses import dataclass, fields
from pathlib import Path

from scaling_canonical_proof import _digest, _integer, _path
from scaling_proof_sequence import StoppedReader

MIB = 1024 * 1024
MAX_BYTES = 256 * MIB
NS = 1_000_000_000
I64 = (1 << 63) - 1
U128 = (1 << 128) - 1


class FactsInputError(ValueError):
    """Closed public geometry failure without original file contents."""


def require(value):
    if not value: raise FactsInputError('native_facts_input_invalid')


def integers(value, expected):
    require(type(value) is expected)
    result = tuple(getattr(value, field.name) for field in fields(expected))
    require(all(type(item) is int for item in result))
    return result


@dataclass(frozen=True, slots=True)
class ReaderBudget:
    """Eight original Core reader limits; an observed height is never a limit."""
    max_committed_blocks: int
    max_store_data_bytes: int
    max_carrier_bytes: int
    max_merge_log_bytes: int
    max_merge_frames: int
    reader_max_output_bytes: int
    max_decode_allocation_bytes: int
    owner_uid: int


def reader_snapshot(value):
    row = integers(value, ReaderBudget)
    maxima = (1_000_000, 2 * 1024 * MIB, 32 * MIB, MAX_BYTES,
              1_000_000, MAX_BYTES, 512 * MIB, (1 << 32) - 1)
    for index, (item, maximum) in enumerate(zip(row, maxima, strict=True)):
        _integer(item, 0 if index == 7 else 1, maximum)
    require(row[4] <= row[0])
    return row


def stopped_reader(store, merge, height, limits):
    """Create the single canonical reader type used by collection and proof export."""
    store, merge = Path(_path(store)), Path(_path(merge))
    require(store != merge)
    _integer(height, 1, limits[0])
    return StoppedReader(store, merge, 1, height, *limits)


@dataclass(frozen=True, slots=True)
class FactsJournalPlan:
    """Original public workload and exact collector/resource timing selection."""
    workload_seed: str
    pair_index: int
    rate_numerator: int
    rate_denominator: int
    warmup_ns: int
    measurement_ns: int
    drain_ns: int
    submission_lag_bound_ns: int
    preparation_lookahead: int
    preparation_concurrency: int
    preparation_ahead_ns: int
    max_submissions: int
    max_in_flight: int
    max_status_requests: int
    poll_interval_ns: int
    journal_max_requests: int
    resource_interval_ns: int
    resource_response_deadline_ns: int
    resource_max_start_lag_ns: int


def journal_snapshot(value, accounts):
    require(type(value) is FactsJournalPlan and type(accounts) is tuple
            and 4 <= len(accounts) <= 64 and len(accounts) % 4 == 0
            and len(set(accounts)) == len(accounts)
            and all(type(item) is str and 0 < len(item) <= 2048
                    and all(33 <= ord(char) <= 126 for char in item) for item in accounts))
    row = tuple(getattr(value, field.name) for field in fields(FactsJournalPlan))
    _digest(row[0])
    for item in row[1:]: require(type(item) is int)
    (seed, pair, numerator, denominator, warmup, measurement, drain, lag,
     lookahead, preparation, ahead, submissions, in_flight, status, poll,
     requests, interval, response, start_lag) = row
    _integer(pair, 1, 5)
    _integer(numerator, 1, U128); _integer(denominator, 1, U128)
    period = NS * denominator
    require(period <= U128 and numerator <= period and 4 * lag * numerator <= period)
    _integer(warmup, 0, I64); _integer(measurement, 1, I64)
    _integer(drain, 1, 300 * NS); _integer(lag, 0, I64)
    for item, maximum in ((lookahead, 4096), (preparation, 32), (submissions, 4096),
                          (in_flight, 16384), (status, 256), (requests, 1_000_000)):
        _integer(item, 1, maximum)
    for item, maximum in ((ahead, 30 * NS), (poll, 10 * NS)):
        _integer(item, 1_000_000, maximum); require(item % 1_000_000 == 0)
    require(warmup * numerator <= U128 and measurement * numerator <= U128)
    counts = tuple((duration * numerator + period - 1) // period for duration in (warmup, measurement))
    require(counts[1] > 0 and all(count % len(accounts) == 0 for count in counts)
            and sum(counts) <= requests and sum(counts) // len(accounts) <= 1024)
    _integer(interval, 2_000_000, 60 * NS)
    _integer(response, 1_000_000, 30 * NS)
    _integer(start_lag, 0, interval // 4)
    require(response <= interval // 2
            and all(item % 1_000_000 == 0 for item in (interval, response, start_lag)))
    require(measurement // interval >= 20 and measurement % interval == drain % interval == 0)
    require(2 <= (measurement + drain) // interval + 1 <= 100_000)
    require(measurement + drain + warmup + drain + ahead + 3 * response + start_lag <= I64)
    return row, counts


@dataclass(frozen=True, slots=True)
class FactsBudget:
    """Original-file, facts assembly and canonical verifier byte/count reservations."""
    manifest_max_bytes: int
    signed_genesis_max_bytes: int
    peer_config_max_bytes: int
    context_max_bytes: int
    source_max_bytes: int
    facts_max_bytes: int
    total_max_bytes: int
    assembly_decode_max_bytes: int
    proof_max_bytes: int
    verification_input_max_bytes: int
    verification_output_max_bytes: int
    max_heights: int
    max_requests: int
    max_leaves_per_carrier: int


def budget_snapshot(value):
    row = integers(value, FactsBudget)
    maxima = (16 * MIB, 32 * MIB, MIB, 8 * MIB, MAX_BYTES, MAX_BYTES,
              MAX_BYTES, 512 * MIB, MAX_BYTES, MAX_BYTES, MAX_BYTES,
              65536, 1_000_000, 1_000_000)
    for item, maximum in zip(row, maxima, strict=True): _integer(item, 1, maximum)
    require(row[4] + row[5] <= row[6] and row[9] + row[10] <= row[8])
    return row


@dataclass(frozen=True, slots=True)
class JournalInput:
    """Independently pinned complete original collector JSONL and its prior cap."""
    path: Path
    sha256: str
    bytes: int
    max_bytes: int


def journal_input_snapshot(value):
    require(type(value) is JournalInput)
    return (Path(_path(value.path)), _digest(value.sha256),
            _integer(value.bytes, 1, _integer(value.max_bytes, 1, MAX_BYTES)), value.max_bytes)
