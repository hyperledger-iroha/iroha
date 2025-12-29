#!/usr/bin/env python3
"""Export the Taikai viewer Grafana dashboard into the repo."""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from urllib import error, request


DEFAULT_UID = "taikai-viewer"
DEFAULT_DEST = Path("dashboards/grafana/taikai_viewer.json")
DEFAULT_TOKEN_ENV = "GRAFANA_TOKEN"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--grafana-url",
        required=True,
        help="Base Grafana URL (e.g. https://grafana.example.com).",
    )
    parser.add_argument(
        "--token",
        help="Grafana API token. If omitted, --token-env is consulted.",
    )
    parser.add_argument(
        "--token-env",
        default=DEFAULT_TOKEN_ENV,
        help=f"Environment variable that stores the Grafana token (default: {DEFAULT_TOKEN_ENV}).",
    )
    parser.add_argument(
        "--uid",
        default=DEFAULT_UID,
        help=f"Dashboard UID to export (default: {DEFAULT_UID}).",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_DEST,
        help=f"Destination path for the exported dashboard (default: {DEFAULT_DEST}).",
    )
    return parser.parse_args()


def resolve_token(args: argparse.Namespace) -> str:
    if args.token:
        return args.token
    token = os.environ.get(args.token_env)
    if not token:
        raise SystemExit(
            f"Grafana token missing; pass --token or set {args.token_env}."
        )
    return token


def fetch_dashboard(base_url: str, token: str, uid: str) -> dict:
    api_url = base_url.rstrip("/") + f"/api/dashboards/uid/{uid}"
    req = request.Request(api_url, headers={"Authorization": f"Bearer {token}"})
    try:
        with request.urlopen(req, timeout=30) as resp:  # nosec B310
            payload = json.load(resp)
    except error.HTTPError as exc:  # pragma: no cover - network errors surfaced to caller
        raise SystemExit(f"Grafana API request failed: {exc}") from exc
    dashboard = payload.get("dashboard")
    if not isinstance(dashboard, dict):
        raise SystemExit("Grafana response missing dashboard payload")

    # Strip volatile fields to keep git history clean.
    for key in ("id", "iteration", "version"):
        dashboard.pop(key, None)
    # Preserve the UID we exported so downstream tooling can verify matches.
    dashboard.setdefault("uid", uid)
    return dashboard


def write_dashboard(dashboard: dict, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(dashboard, indent=2) + "\n", encoding="utf-8")
    print(f"[taikai-viewer] exported dashboard to {destination}")


def main() -> None:
    args = parse_args()
    token = resolve_token(args)
    dashboard = fetch_dashboard(args.grafana_url, token, args.uid)
    write_dashboard(dashboard, args.out)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
