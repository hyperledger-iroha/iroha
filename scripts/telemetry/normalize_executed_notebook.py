#!/usr/bin/env python3
"""Canonicalize volatile execution metadata in a generated notebook."""

from __future__ import annotations

import argparse
import json
import os
import stat
import tempfile
from pathlib import Path


MAX_NOTEBOOK_BYTES = 64 * 1024 * 1024


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--notebook", required=True, type=Path)
    parser.add_argument("--source-date-epoch", required=True, type=int)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.source_date_epoch < 0:
        raise SystemExit("--source-date-epoch must be non-negative")
    path = args.notebook
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise SystemExit(f"unable to open executed notebook safely: {exc}") from exc
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise SystemExit("executed notebook must be a regular file")
        if before.st_size > MAX_NOTEBOOK_BYTES:
            raise SystemExit("executed notebook exceeds the 64 MiB limit")
        raw = bytearray()
        while len(raw) <= MAX_NOTEBOOK_BYTES:
            chunk = os.read(descriptor, min(1024 * 1024, MAX_NOTEBOOK_BYTES + 1 - len(raw)))
            if not chunk:
                break
            raw.extend(chunk)
        if len(raw) > MAX_NOTEBOOK_BYTES:
            raise SystemExit("executed notebook exceeds the 64 MiB limit")
    finally:
        os.close(descriptor)

    try:
        notebook = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise SystemExit(f"executed notebook is not valid JSON: {exc}") from exc
    if not isinstance(notebook, dict) or not isinstance(notebook.get("cells"), list):
        raise SystemExit("executed notebook has an invalid document shape")

    metadata = notebook.setdefault("metadata", {})
    if not isinstance(metadata, dict):
        raise SystemExit("executed notebook metadata must be an object")
    metadata.pop("papermill", None)
    metadata["iroha_release"] = {
        "source_date_epoch": args.source_date_epoch,
    }
    for cell in notebook["cells"]:
        if not isinstance(cell, dict):
            raise SystemExit("executed notebook cell must be an object")
        cell_metadata = cell.setdefault("metadata", {})
        if not isinstance(cell_metadata, dict):
            raise SystemExit("executed notebook cell metadata must be an object")
        cell_metadata.pop("execution", None)
        cell_metadata.pop("papermill", None)
        outputs = cell.get("outputs", [])
        if not isinstance(outputs, list):
            raise SystemExit("executed notebook cell outputs must be an array")
        for output in outputs:
            if not isinstance(output, dict):
                raise SystemExit("executed notebook output must be an object")
            output_metadata = output.get("metadata")
            if isinstance(output_metadata, dict):
                output_metadata.pop("execution", None)
                output_metadata.pop("papermill", None)

    normalized = (
        json.dumps(
            notebook,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")
    current = path.lstat()
    if (
        current.st_dev != before.st_dev
        or current.st_ino != before.st_ino
        or current.st_size != before.st_size
        or current.st_mtime_ns != before.st_mtime_ns
    ):
        raise SystemExit("executed notebook changed while it was normalized")

    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=path.parent,
            prefix=f".{path.name}.",
            delete=False,
        ) as stream:
            temporary_name = stream.name
            os.fchmod(stream.fileno(), 0o644)
            stream.write(normalized)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_name, path)
        temporary_name = None
        os.utime(
            path,
            (args.source_date_epoch, args.source_date_epoch),
            follow_symlinks=False,
        )
    finally:
        if temporary_name is not None:
            try:
                os.unlink(temporary_name)
            except FileNotFoundError:
                pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
