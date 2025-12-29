#!/usr/bin/env python3
"""Download or verify the canonical ADDR-2 account-address fixture.

SDK release tooling frequently needs to grab the latest
`fixtures/account/address_vectors.json` bundle without copying it by hand.
This helper provides a tiny CLI that can fetch the JSON from a remote source
or a local path, write it to disk, and report whether the local copy matches.

Example usage:

```bash
# Download the fixture from the default GitHub raw URL
python3 scripts/account_fixture_helper.py fetch \
  --output path/to/sdk/fixtures/address_vectors.json

# Verify that a local SDK copy matches the canonical fixture
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/fixtures/address_vectors.json
```
"""

from __future__ import annotations

import argparse
import hashlib
import os
import sys
from pathlib import Path
from typing import Optional
from urllib import parse, request

DEFAULT_REMOTE_URL = os.environ.get(
    "IROHA_ACCOUNT_FIXTURE_URL",
    "https://raw.githubusercontent.com/hyperledger-iroha/iroha/main/"
    "fixtures/account/address_vectors.json",
)


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


DEFAULT_FIXTURE_PATH = _repo_root() / "fixtures" / "account" / "address_vectors.json"


def is_url(value: str) -> bool:
    """Return True if the supplied string looks like an absolute URL."""
    parsed = parse.urlparse(value)
    if parsed.scheme and parsed.netloc:
        return True
    # Support file:// URLs so SDK automation can reference local artifacts.
    return parsed.scheme == "file"


def fetch_fixture_bytes(source: str) -> bytes:
    """Load fixture bytes from an HTTP(S) URL or filesystem path."""
    if not source:
        raise ValueError("fixture source cannot be empty")
    if is_url(source):
        with request.urlopen(source) as response:  # type: ignore[call-arg]
            payload = response.read()
            status = getattr(response, "status", 200)
            if status and int(status) >= 400:
                raise RuntimeError(f"failed to download fixture ({status}) from {source}")
            return payload
    path = Path(source).expanduser()
    return path.read_bytes()


def write_bytes_if_changed(target: Path, data: bytes, *, force: bool = False) -> bool:
    """Write `data` to `target` if it differs. Returns True when the file changes."""
    if not force and target.exists():
        current = target.read_bytes()
        if current == data:
            return False
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    return True


def compare_remote_bytes(target: Path, remote: bytes) -> bool:
    """Return True when `target` exists and matches the supplied bytes."""
    try:
        local = target.read_bytes()
    except FileNotFoundError:
        return False
    return local == remote


def sha256_hex(data: bytes) -> str:
    """Return the SHA-256 hex digest for `data`."""
    return hashlib.sha256(data).hexdigest()


def _default_source(source: Optional[str]) -> str:
    return source or DEFAULT_REMOTE_URL


def _default_target(path: Optional[str]) -> Path:
    if path:
        return Path(path).expanduser()
    return DEFAULT_FIXTURE_PATH


def _default_metrics_label(target: Path, override: Optional[str]) -> Optional[str]:
    if override is not None:
        label = override.strip()
    else:
        label = target.name.strip() or str(target).strip()
    if not label:
        return None
    return label


def _escape_label(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _write_metrics(
    output: Path,
    label: str,
    status_ok: bool,
    remote_sha: str,
    local_sha: Optional[str],
) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    label_escaped = _escape_label(label)
    lines = [
        "# HELP account_address_fixture_check_status "
        "1 when the local copy matches the canonical ADDR-2 fixture.",
        "# TYPE account_address_fixture_check_status gauge",
        f'account_address_fixture_check_status{{target="{label_escaped}"}} '
        f'{"1" if status_ok else "0"}',
        "# HELP account_address_fixture_remote_info Metadata for the canonical fixture digest.",
        "# TYPE account_address_fixture_remote_info gauge",
        (
            "account_address_fixture_remote_info"
            f'{{target="{label_escaped}",sha256="{remote_sha}"}} 1'
        ),
    ]
    if local_sha is not None:
        lines.extend(
            [
                "# HELP account_address_fixture_local_info Metadata for the checked file digest.",
                "# TYPE account_address_fixture_local_info gauge",
                (
                    "account_address_fixture_local_info"
                    f'{{target="{label_escaped}",sha256="{local_sha}"}} 1'
                ),
            ]
        )
    else:
        lines.extend(
            [
                "# HELP account_address_fixture_local_missing "
                "1 when the local fixture file was not found.",
                "# TYPE account_address_fixture_local_missing gauge",
                f'account_address_fixture_local_missing{{target="{label_escaped}"}} 1',
            ]
        )
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _emit_metrics_if_requested(
    args: argparse.Namespace,
    *,
    target: Path,
    status_ok: bool,
    remote_sha: str,
    local_sha: Optional[str],
) -> None:
    metrics_path = getattr(args, "metrics_out", None)
    if not metrics_path:
        return
    label = _default_metrics_label(target, getattr(args, "metrics_label", None))
    if not label:
        raise RuntimeError("--metrics-label must not be empty when provided")
    _write_metrics(Path(metrics_path).expanduser(), label, status_ok, remote_sha, local_sha)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Download or verify the ADDR-2 account address fixture.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    fetch_parser = subparsers.add_parser("fetch", help="Download the fixture to disk.")
    fetch_parser.add_argument(
        "--source",
        metavar="URL_OR_PATH",
        help="Override the fixture source (defaults to the canonical GitHub raw URL).",
    )
    fetch_parser.add_argument(
        "--output",
        metavar="PATH",
        help=f"Path to write (defaults to {DEFAULT_FIXTURE_PATH})",
    )
    fetch_parser.add_argument(
        "--force",
        action="store_true",
        help="Write even if the current file matches the downloaded content.",
    )
    fetch_parser.add_argument(
        "--stdout",
        action="store_true",
        help="Print the downloaded JSON to stdout instead of writing a file.",
    )

    check_parser = subparsers.add_parser(
        "check",
        help="Compare the local file against the canonical source and exit non-zero on drift.",
    )
    check_parser.add_argument(
        "--source",
        metavar="URL_OR_PATH",
        help="Override the fixture source when comparing.",
    )
    check_parser.add_argument(
        "--target",
        metavar="PATH",
        help=f"Path to verify (defaults to {DEFAULT_FIXTURE_PATH})",
    )
    check_parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress success output; still prints drift details on failure.",
    )
    check_parser.add_argument(
        "--metrics-out",
        metavar="PATH",
        help="Write Prometheus text-format metrics capturing the check status.",
    )
    check_parser.add_argument(
        "--metrics-label",
        metavar="LABEL",
        help="Override the Prometheus `target` label (defaults to the target filename).",
    )

    return parser


def _handle_fetch(args: argparse.Namespace) -> int:
    source = _default_source(args.source)
    output = _default_target(args.output)
    payload = fetch_fixture_bytes(source)
    digest = sha256_hex(payload)
    if args.stdout:
        sys.stdout.buffer.write(payload)
        return 0
    changed = write_bytes_if_changed(output, payload, force=args.force)
    if changed:
        print(f"Wrote {len(payload)} bytes to {output} (sha256={digest}).")
    else:
        print(f"{output} already up to date (sha256={digest}).")
    return 0


def _handle_check(args: argparse.Namespace) -> int:
    source = _default_source(args.source)
    target = _default_target(args.target)
    payload = fetch_fixture_bytes(source)
    digest_remote = sha256_hex(payload)
    try:
        local_bytes = target.read_bytes()
        digest_local = sha256_hex(local_bytes)
    except FileNotFoundError:
        _emit_metrics_if_requested(
            args,
            target=target,
            status_ok=False,
            remote_sha=digest_remote,
            local_sha=None,
        )
        print(
            f"{target} is missing while {source} reports sha256={digest_remote}.",
            file=sys.stderr,
        )
        return 2

    if local_bytes == payload:
        _emit_metrics_if_requested(
            args,
            target=target,
            status_ok=True,
            remote_sha=digest_remote,
            local_sha=digest_local,
        )
        if not args.quiet:
            print(f"{target} matches {source} (sha256={digest_remote}).")
        return 0
    _emit_metrics_if_requested(
        args,
        target=target,
        status_ok=False,
        remote_sha=digest_remote,
        local_sha=digest_local,
    )
    print(
        f"{target} does not match {source}.\n"
        f" local sha256={digest_local}\n remote sha256={digest_remote}",
        file=sys.stderr,
    )
    return 1


def main(argv: Optional[list[str]] = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    if args.command == "fetch":
        return _handle_fetch(args)
    if args.command == "check":
        return _handle_check(args)
    parser.error(f"unknown command {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
