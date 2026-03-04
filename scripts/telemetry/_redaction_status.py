#!/usr/bin/env python3
"""
Helper utilities for Android telemetry redaction scripts.

The helper centralises reading/writing the local status cache and fetching
status JSON from HTTP endpoints so both the check and injection helpers stay in
sync.
"""

from __future__ import annotations

import datetime as dt
import json
import urllib.request
from pathlib import Path
from typing import Any, Dict, Iterable, Optional

DEFAULT_STATUS_PATH = Path("artifacts/android/telemetry/status.json")


def _now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0)


def now_iso() -> str:
    """Return the current UTC timestamp in ISO-8601 format with a `Z` suffix."""
    return _now().isoformat().replace("+00:00", "Z")


def ensure_parent(path: Path) -> None:
    """Ensure the parent directory for ``path`` exists."""
    path.parent.mkdir(parents=True, exist_ok=True)


def default_status() -> Dict[str, Any]:
    """Return a fresh status structure."""
    return {
        "timestamp": now_iso(),
        "hashed_authorities": 0,
        "mismatched_authorities": 0,
        "recent_failures": [],
        "salt": {"epoch": None, "rotation_id": None},
        "exporters": [],
    }


def normalise_status(data: Dict[str, Any]) -> Dict[str, Any]:
    """Ensure the status payload contains the expected keys/types."""
    result = default_status()
    result.update({k: v for k, v in data.items() if k in result})
    # Preserve nested structures if present.
    if "salt" in data and isinstance(data["salt"], dict):
        result["salt"].update(data["salt"])
    if "exporters" in data and isinstance(data["exporters"], list):
        result["exporters"] = list(data["exporters"])
    if "recent_failures" in data and isinstance(data["recent_failures"], list):
        normalised_failures = []
        for item in data["recent_failures"]:
            if not isinstance(item, dict):
                continue
            entry = {
                "authority": item.get("authority"),
                "reason": item.get("reason", "unknown"),
                "count": int(item.get("count", 1) or 1),
                "timestamp": item.get("timestamp") or now_iso(),
            }
            if "note" in item:
                entry["note"] = item["note"]
            normalised_failures.append(entry)
        result["recent_failures"] = normalised_failures
    # Preserve other keys that callers may care about (e.g., exporter stats).
    for key in ("metrics",):
        if key in data:
            result[key] = data[key]
    return result


def read_status_file(path: Path, *, create: bool = False) -> Dict[str, Any]:
    """Read a status file, optionally creating it when missing."""
    if path.exists():
        try:
            return normalise_status(json.loads(path.read_text(encoding="utf-8")))
        except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
            raise ValueError(f"status file {path} is not valid JSON") from exc
    if not create:
        raise FileNotFoundError(path)
    ensure_parent(path)
    data = default_status()
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    return data


def write_status_file(path: Path, data: Dict[str, Any]) -> None:
    """Persist a status payload to disk."""
    ensure_parent(path)
    payload = normalise_status(data)
    payload["timestamp"] = now_iso()
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def fetch_status_from_url(
    url: str, *, timeout: float = 5.0, headers: Optional[Dict[str, str]] = None
) -> Dict[str, Any]:
    """Fetch status JSON from an HTTP endpoint."""
    request = urllib.request.Request(url, headers=headers or {})
    with urllib.request.urlopen(request, timeout=timeout) as response:  # nosec B310
        content_type = response.headers.get("Content-Type", "")
        body = response.read().decode("utf-8")
    if "json" not in content_type and not body.strip().startswith("{"):
        raise ValueError(f"endpoint {url} did not return JSON")
    try:
        data = json.loads(body)
    except json.JSONDecodeError as exc:
        raise ValueError(f"endpoint {url} returned invalid JSON") from exc
    return normalise_status(data)


def update_failure_entries(
    data: Dict[str, Any],
    *,
    authority: str,
    reason: str,
    count: int = 1,
    note: Optional[str] = None,
) -> Dict[str, Any]:
    """Append a failure entry to the status payload."""
    status = normalise_status(data)
    entry = {
        "authority": authority,
        "reason": reason,
        "count": max(1, int(count)),
        "timestamp": now_iso(),
    }
    if note:
        entry["note"] = note
    failures: Iterable[Dict[str, Any]] = status.get("recent_failures", [])
    status["recent_failures"] = list(failures) + [entry]
    status["mismatched_authorities"] = (
        int(status.get("mismatched_authorities", 0)) + entry["count"]
    )
    # Ensure hashed count is at least the mismatch count.
    hashed = int(status.get("hashed_authorities", 0))
    if hashed < status["mismatched_authorities"]:
        status["hashed_authorities"] = status["mismatched_authorities"]
    return status


def clear_failures(data: Dict[str, Any]) -> Dict[str, Any]:
    """Reset mismatch counters and remove recorded failures."""
    status = normalise_status(data)
    status["mismatched_authorities"] = 0
    status["recent_failures"] = []
    return status
