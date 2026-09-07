#!/usr/bin/env python3
"""Check or write the SDK operation inventory from Torii's compiled route catalog.

Requires Python 3.10+ and the repository's pinned Rust toolchain on PATH. No node
binary, Cargo build, network access, or environment variables are needed. The
default is a read-only drift check; --write explicitly refreshes the inventory.
The std-only exporter compiles in a temporary directory removed on completion.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = Path("specs/sdk_operation_inventory.tsv")


def generate(root: Path) -> bytes:
    """Compile the authoritative descriptors and return their deterministic TSV."""
    with tempfile.TemporaryDirectory(prefix="iroha-sdk-operations-") as temporary:
        binary = Path(temporary) / "sdk-operation-inventory"
        subprocess.run(
            [
                "rustc", "--edition=2024", "--crate-name", "sdk_operation_inventory",
                str(root / "scripts/sdk_operation_inventory.rs"), "-o", str(binary),
            ],
            cwd=root, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        return subprocess.run(
            [str(binary)], cwd=root, check=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        ).stdout


def main(argv: list[str] | None = None) -> int:
    """Keep the checked-in operation inventory exact without implicit rewrites."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="refresh the reviewed inventory")
    args = parser.parse_args(argv)
    try:
        observed = generate(ROOT)
        path = ROOT / INVENTORY
        if args.write:
            path.write_bytes(observed)
        elif path.read_bytes() != observed:
            print("SDK operation inventory differs from the canonical Torii routes; "
                  "review the change and run with --write", file=sys.stderr)
            return 1
    except subprocess.CalledProcessError as error:
        print(error.stderr.decode("utf-8", errors="replace"), file=sys.stderr)
        return 2
    except OSError as error:
        print(f"SDK operation inventory failed: {error}", file=sys.stderr)
        return 2
    print(f"sdk_operation_inventory: routes={len(observed.splitlines()) - 2}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
