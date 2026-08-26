#!/usr/bin/env python3
"""Export the Taikai viewer Grafana dashboard into the repo."""
from __future__ import annotations

import argparse
import json
import os
import stat
import sys
import tempfile
from pathlib import Path
from urllib import error, parse, request


DEFAULT_UID = "taikai-viewer"
DEFAULT_DEST = Path("dashboards/grafana/taikai_viewer.json")
DEFAULT_TOKEN_ENV = "GRAFANA_TOKEN"
MAX_DASHBOARD_BYTES = 16 * 1024 * 1024


class _NoRedirectHandler(request.HTTPRedirectHandler):
    """Keep bearer credentials on the explicitly configured origin."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: ANN001
        return None


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
    parsed_base = parse.urlsplit(base_url)
    if (
        parsed_base.scheme not in {"http", "https"}
        or not parsed_base.netloc
        or parsed_base.query
        or parsed_base.fragment
    ):
        raise SystemExit("Grafana URL must be an HTTP(S) base URL without query or fragment")
    if not uid or uid != uid.strip() or any(character.isspace() for character in uid):
        raise SystemExit("Grafana dashboard UID must be non-empty and contain no whitespace")
    encoded_uid = parse.quote(uid, safe="")
    api_url = base_url.rstrip("/") + f"/api/dashboards/uid/{encoded_uid}"
    req = request.Request(api_url, headers={"Authorization": f"Bearer {token}"})
    opener = request.build_opener(_NoRedirectHandler())
    try:
        with opener.open(req, timeout=30) as resp:  # nosec B310
            content_length = resp.headers.get("Content-Length")
            if content_length is not None:
                try:
                    declared_length = int(content_length)
                except ValueError as exc:
                    raise SystemExit("Grafana response has invalid Content-Length") from exc
                if declared_length < 0 or declared_length > MAX_DASHBOARD_BYTES:
                    raise SystemExit(
                        f"Grafana dashboard exceeds {MAX_DASHBOARD_BYTES} bytes"
                    )
            raw = resp.read(MAX_DASHBOARD_BYTES + 1)
    except error.HTTPError as exc:  # pragma: no cover - network errors surfaced to caller
        raise SystemExit(f"Grafana API request failed: {exc}") from exc
    if len(raw) > MAX_DASHBOARD_BYTES:
        raise SystemExit(f"Grafana dashboard exceeds {MAX_DASHBOARD_BYTES} bytes")
    try:
        payload = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise SystemExit(f"Grafana response is not valid JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise SystemExit("Grafana response must be a JSON object")
    dashboard = payload.get("dashboard")
    if not isinstance(dashboard, dict):
        raise SystemExit("Grafana response missing dashboard payload")

    # Strip volatile fields to keep git history clean.
    for key in ("id", "iteration", "version"):
        dashboard.pop(key, None)
    actual_uid = dashboard.get("uid")
    if actual_uid is not None and actual_uid != uid:
        raise SystemExit(
            f"Grafana response dashboard UID `{actual_uid}` does not match requested `{uid}`"
        )
    # Preserve the UID we exported so downstream tooling can verify matches.
    dashboard["uid"] = uid
    return dashboard


def write_dashboard(dashboard: dict, destination: Path) -> None:
    destination = destination.parent.resolve(strict=False) / destination.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.is_symlink() or (
        destination.exists() and not destination.is_file()
    ):
        raise SystemExit(
            f"dashboard destination must be a regular file or absent: {destination}"
        )
    descriptor, temporary_name = tempfile.mkstemp(
        dir=destination.parent, prefix=f".{destination.name}.tmp-"
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            json.dump(dashboard, output, indent=2)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        mode = (
            stat.S_IMODE(destination.stat().st_mode)
            if destination.exists()
            else 0o644
        )
        os.chmod(temporary, mode)
        os.replace(temporary, destination)
        try:
            directory_fd = os.open(destination.parent, os.O_RDONLY)
        except OSError:
            directory_fd = None
        if directory_fd is not None:
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
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
