"""Canonical script imports and this suite's explicitly owned helper directory."""

from pathlib import Path
import os
import sys

import pytest


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [
    str(REPOSITORY_ROOT),
    str(REPOSITORY_ROOT / "scripts"),
    str(Path(__file__).resolve().parent),
]


@pytest.fixture(autouse=True)
def _restore_process_umask():
    """Restore the process umask after each test.

    Operator CLIs set a private umask in main(); tests that call main() in-process
    must not leak it into later tests that share the worker process.
    """
    previous = os.umask(0o077)
    os.umask(previous)
    yield
    os.umask(previous)
