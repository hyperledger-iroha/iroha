#!/usr/bin/env python3
"""Inspect the exact user-owned source closure for a local Taira reset."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from . import prepare_taira_empty_reset_bundle as reset_bundle
except ImportError:
    import prepare_taira_empty_reset_bundle as reset_bundle


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument(
        "--digest-only",
        action="store_true",
        help="print only the lowercase SHA-256 identity",
    )
    args = parser.parse_args(argv)
    manifest, digest = reset_bundle.local_testnet_source_closure()
    if args.digest_only:
        print(digest)
    else:
        sys.stdout.buffer.write(
            reset_bundle.canonical_json_bytes({**manifest, "sha256": digest})
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
