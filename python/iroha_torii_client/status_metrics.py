"""Internal status-delta calculation for the low-level Torii client."""

from __future__ import annotations

from typing import Any, Dict, Optional


def compute_status_metric_values(
    previous: Optional[Any],
    current: Any,
) -> Dict[str, Any]:
    """Return constructor values for one immutable status-metrics snapshot."""

    queue_delta = 0 if previous is None else current.queue_size - previous.queue_size
    approved_delta = (
        0 if previous is None else max(0, current.txs_approved - previous.txs_approved)
    )
    rejected_delta = (
        0 if previous is None else max(0, current.txs_rejected - previous.txs_rejected)
    )
    view_delta = (
        0 if previous is None else max(0, current.view_changes - previous.view_changes)
    )
    has_activity = any(
        value
        for value in (
            queue_delta,
            approved_delta,
            rejected_delta,
            view_delta,
        )
    )
    return {
        "commit_latency_ms": current.commit_time_ms,
        "queue_size": current.queue_size,
        "queue_queued": current.queue_queued,
        "queue_inflight": current.queue_inflight,
        "queue_delta": queue_delta,
        "time_since_last_block_ms": current.time_since_last_block_ms,
        "time_since_last_non_empty_block_ms": current.time_since_last_non_empty_block_ms,
        "tx_approved_delta": approved_delta,
        "tx_rejected_delta": rejected_delta,
        "view_change_delta": view_delta,
        "has_activity": bool(has_activity),
    }
