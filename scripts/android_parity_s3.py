#!/usr/bin/env python3
"""Upload Android parity summaries to S3 for AND3 dashboards."""

from __future__ import annotations

import argparse
import shlex
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


@dataclass
class UploadSpec:
    local: Path
    remote_relative: str


def normalize_prefix(prefix: str) -> str:
    prefix = prefix.strip()
    if not prefix:
        raise ValueError("S3 prefix must not be empty")
    if not prefix.startswith("s3://"):
        raise ValueError("S3 prefix must start with 's3://'")
    return prefix.rstrip("/")


def normalize_relative_path(relative: str) -> str:
    relative = relative.strip().lstrip("/")
    if not relative:
        raise ValueError("Remote key must not be empty")
    return relative


def build_remote_uri(prefix: str, relative: str) -> str:
    return f"{normalize_prefix(prefix)}/{normalize_relative_path(relative)}"


def parse_upload_spec(spec: str) -> UploadSpec:
    if ":" not in spec:
        raise ValueError("upload spec must be formatted as '<local>:<remote>'")
    local_str, remote = spec.split(":", 1)
    local_path = Path(local_str).expanduser()
    if not remote.strip():
        raise ValueError("remote key must not be empty")
    return UploadSpec(local=local_path, remote_relative=remote)


def perform_uploads(
    prefix: str,
    uploads: Iterable[UploadSpec],
    cli: Sequence[str],
    extra_args: Sequence[str],
) -> None:
    normalized_prefix = normalize_prefix(prefix)
    for spec in uploads:
        if not spec.local.is_file():
            raise FileNotFoundError(f"upload source not found: {spec.local}")
        remote_uri = build_remote_uri(normalized_prefix, spec.remote_relative)
        command = list(cli) + ["s3", "cp", str(spec.local), remote_uri]
        command.extend(extra_args)
        print(
            f"[android-parity-s3] uploading {spec.local} to {remote_uri} via {' '.join(cli)}",
            flush=True,
        )
        subprocess.run(command, check=True)


def parse_cli(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--prefix",
        required=True,
        help="S3 prefix (e.g. s3://bucket/android/parity)",
    )
    parser.add_argument(
        "--upload",
        action="append",
        required=True,
        metavar="LOCAL:REMOTE",
        help="Local file and remote key pair to upload",
    )
    parser.add_argument(
        "--cli",
        default="aws",
        help="Command used for uploads (default: aws)",
    )
    parser.add_argument(
        "--extra-arg",
        action="append",
        default=[],
        help="Extra argument appended to the upload command (repeatable)",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_cli(argv)
    uploads = [parse_upload_spec(spec) for spec in args.upload]
    cli = shlex.split(args.cli)
    extra_args = list(args.extra_arg)
    perform_uploads(args.prefix, uploads, cli, extra_args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
