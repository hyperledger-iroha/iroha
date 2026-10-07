"""Explicit offline installation of a genuinely new private E1 counter store.

Serving never imports or invokes this command. It creates neither a directory nor a
replacement store, and delegates the complete durable schema operation to its sole owner.
"""
import argparse
import os
from pathlib import Path
import sqlite3
import sys

from .attestation import VerificationUnavailable
from .wallet_enrollment_store import E1CounterStore


def initialize_new(directory: Path) -> None:
    """Initialize once in an existing, empty, private, canonical directory.

    Every created original is retained on failure. Missing or damaged storage is never
    interpreted as permission to repair, reset counters, or retry a fresh generation.
    """
    if not directory.is_absolute() or directory.resolve(strict=True) != directory:
        raise VerificationUnavailable("canonical existing private directory required")
    if not all(hasattr(os, flag) for flag in ("O_DIRECTORY", "O_NOFOLLOW", "O_CLOEXEC")):
        raise VerificationUnavailable("descriptor-relative installation is unavailable")
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        # Reuse the serving store's exact directory custody primitive. The empty-directory
        # precondition is an installation constraint, never a serving absence heuristic.
        E1CounterStore._directory(directory, descriptor)
        if os.listdir(descriptor):
            raise FileExistsError("new private store directory must be empty")
        owner = E1CounterStore.initialize(directory, descriptor)
        try:
            owner.recheck()
        finally:
            owner.close()
        E1CounterStore._directory(directory, descriptor)
    finally:
        os.close(descriptor)


def main(argv=None) -> int:
    """Run the explicit installation action; errors never remove or alter retained originals."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--store-directory", type=Path, required=True,
                        help="absolute existing empty private directory owned by the service UID")
    arguments = parser.parse_args(argv)
    try:
        initialize_new(arguments.store_directory)
    except (OSError, ValueError, sqlite3.Error, VerificationUnavailable):
        print("private E1 store initialization refused; retained originals were not repaired",
              file=sys.stderr)
        return 1
    print("private E1 store initialized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
