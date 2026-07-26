#!/usr/bin/env python3
"""Validate Soracloud production observability evidence."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_METRICS = (
    "signed_auth_failures",
    "body_limit_rejections",
    "rate_limited_requests",
    "inflight_limit_rejections",
    "runtime_hydration_lag",
    "inrou_lifecycle",
    "lease_volume_pressure",
    "cache_pressure",
    "disk_pressure",
    "egress_usage",
    "model_host_stale_heartbeats",
    "hf_fallback_use",
    "private_session_failures",
)

REQUIRED_STATUS_FIELDS = (
    "config_posture",
    "feature_flags",
    "route_exposure",
    "runtime_capabilities",
    "runtime_hydration",
    "inrou_lifecycle",
    "lease_volume_pressure",
    "cache_pressure",
    "disk_pressure",
    "egress_usage",
    "model_host_heartbeats",
    "hf_fallback_use",
    "private_session_failures",
)

REQUIRED_ALERTS = (
    "signed_auth_failures",
    "body_limit_rejections",
    "rate_limited_requests",
    "inflight_limit_rejections",
    "runtime_hydration_lag",
    "lease_volume_pressure",
    "disk_pressure",
    "egress_usage",
    "model_host_stale_heartbeats",
    "hf_fallback_use",
    "private_session_failures",
)


def require_object(value: Any, path: str, errors: list[str]) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    errors.append(f"{path} must be an object")
    return {}


def require_non_empty_string(value: Any, path: str, errors: list[str]) -> None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{path} must be a non-empty string")


def require_true(value: Any, path: str, errors: list[str]) -> None:
    if value is not True:
        errors.append(f"{path} must be true")


def validate_named_records(
    payload: dict[str, Any],
    section_name: str,
    required_names: tuple[str, ...],
    errors: list[str],
    *,
    require_source: bool,
) -> None:
    section = require_object(payload.get(section_name), section_name, errors)
    for name in required_names:
        record = require_object(section.get(name), f"{section_name}.{name}", errors)
        require_true(record.get("present"), f"{section_name}.{name}.present", errors)
        if require_source:
            require_non_empty_string(record.get("source"), f"{section_name}.{name}.source", errors)


def validate_alerts(payload: dict[str, Any], errors: list[str]) -> None:
    alerts = require_object(payload.get("alerts"), "alerts", errors)
    for name in REQUIRED_ALERTS:
        alert = require_object(alerts.get(name), f"alerts.{name}", errors)
        require_true(alert.get("enabled"), f"alerts.{name}.enabled", errors)
        require_non_empty_string(alert.get("severity"), f"alerts.{name}.severity", errors)
        require_non_empty_string(alert.get("runbook"), f"alerts.{name}.runbook", errors)


def validate_dashboards(payload: dict[str, Any], errors: list[str]) -> None:
    dashboards = payload.get("dashboards")
    if not isinstance(dashboards, list) or not dashboards:
        errors.append("dashboards must be a non-empty array")
        return
    for index, dashboard in enumerate(dashboards):
        record = require_object(dashboard, f"dashboards[{index}]", errors)
        require_non_empty_string(record.get("name"), f"dashboards[{index}].name", errors)
        require_non_empty_string(record.get("url"), f"dashboards[{index}].url", errors)


def validate_evidence(payload: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    require_non_empty_string(payload.get("generated_at"), "generated_at", errors)
    deployment = require_object(payload.get("deployment"), "deployment", errors)
    require_non_empty_string(deployment.get("name"), "deployment.name", errors)
    require_non_empty_string(deployment.get("environment"), "deployment.environment", errors)
    require_non_empty_string(deployment.get("operator"), "deployment.operator", errors)
    validate_named_records(
        payload,
        "metrics",
        REQUIRED_METRICS,
        errors,
        require_source=True,
    )
    validate_named_records(
        payload,
        "status_fields",
        REQUIRED_STATUS_FIELDS,
        errors,
        require_source=True,
    )
    validate_alerts(payload, errors)
    validate_dashboards(payload, errors)
    return errors


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError("observability evidence root must be a JSON object")
    return payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate Soracloud production observability evidence JSON."
    )
    parser.add_argument("--evidence", required=True, type=Path, help="Evidence JSON path")
    args = parser.parse_args(argv)

    try:
        payload = load_json(args.evidence)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"ERROR: failed to load {args.evidence}: {error}", file=sys.stderr)
        return 1

    errors = validate_evidence(payload)
    if errors:
        print("ERROR: Soracloud observability evidence is incomplete:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(
        "Soracloud observability evidence covers "
        f"{len(REQUIRED_METRICS)} metrics, "
        f"{len(REQUIRED_STATUS_FIELDS)} status fields, "
        f"{len(REQUIRED_ALERTS)} alerts, and "
        f"{len(payload.get('dashboards', []))} dashboard(s)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
