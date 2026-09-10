"""Synthetic opaque byte retention for source-only component fixtures.

These bytes intentionally are not a signed Norito transaction. Real publisher,
scanner and replay tests can authenticate their retention; the compiled canonical
verifier must reject them. No test here claims signature or release validation.
"""
import copy
import hashlib
import json


def command(index, plan, tx_hash, raw):
    """Build the documented protocol independently of the reader implementation."""
    digest = hashlib.sha256(raw).hexdigest()
    count = (len(raw) + 4095) // 4096
    summary = dict(index=index, hash=tx_hash, byte_length=len(raw), canonical_sha256=digest, chunk_count=count)
    rows = [dict(event='signed_request_begin', plan=copy.deepcopy(plan), encoding='norito.canonical.signed_transaction.v1', **summary)]
    rows.extend(dict(event='signed_request_chunk', index=index, chunk_index=ordinal, offset=offset,
                     bytes_hex=raw[offset:offset + 4096].hex())
                for ordinal, offset in enumerate(range(0, len(raw), 4096)))
    rows.append(dict(event='signed_request_retained', **summary))
    return rows


def add_retention(events):
    """Construct truthful ordering once, never repair a mutation at save/replay."""
    events = copy.deepcopy(events)
    clock = next(i for i, row in enumerate(events) if row['event'] == 'clock_started')
    finals = [row for row in events if row['event'] == 'request_final']
    if not any(row['event'] == 'scheduled' for row in events):
        scheduled = [dict(event='scheduled', index=i, plan=copy.deepcopy(row['plan'])) for i, row in enumerate(finals)]
        events[clock:clock] = scheduled
        clock += len(scheduled)
    prefix, tail, timed = events[:clock + 1], [], []
    for ordinal, row in enumerate(events[clock + 1:]):
        if row['event'] == 'resource_request':
            timed.append((row['start_offset_ns'], ordinal, [row]))
        elif row['event'] in ('resource_observation', 'resource_collection_finished'):
            timed.append((row['end_offset_ns'], ordinal, [row]))
        else:
            tail.append(row)
    for index, row in enumerate(finals):
        plan, tx_hash, offer = row['plan'], row['hash'], row['offer_offset_ns']
        raw = b'SYNTHETIC UNVERIFIED TRANSACTION\0' + json.dumps({'plan': plan, 'hash': tx_hash}, sort_keys=True, separators=(',', ':')).encode()
        prepared = max(prefix[-1]['initial_offset_ns'], offer - 1)
        group = command(index, plan, tx_hash, raw)
        group.append(dict(event='prepared', index=index, hash=tx_hash, offset_ns=prepared))
        timed.append((prepared, len(events) + 2 * index, group))
        timed.append((offer, len(events) + 2 * index + 1,
                      [dict(event='offer', index=index, hash=tx_hash, offset_ns=offer)]))
    return prefix + [row for _, _, group in sorted(timed) for row in group] + tail
