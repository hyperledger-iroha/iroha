"""Owned workspace directories for release custody tests."""
from __future__ import annotations

import os
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path


@contextmanager
def owned_release_workspace(prefix: str) -> Iterator[Path]:
    """Keep custody fixtures below the checkout's owned test output directory.

    These contracts reject every shared writable ancestor, so pytest's ambient
    system temporary root cannot supply their positive and race fixtures.
    TemporaryDirectory removes only this test's exclusively created directory.
    Owned setup uses 077; test bodies use ordinary 022 so explicit safe 0755
    anchors and 0644 artifacts retain their intended coverage. The caller mask
    is restored after the test, including failures during creation or cleanup.
    """
    original_umask = os.umask(0o077)
    try:
        parent = Path(__file__).resolve().parents[2]
        for component in ("target", "unit-tests", "script-tests"):
            parent /= component
            parent.mkdir(mode=0o700, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=prefix, dir=parent) as directory:
            os.umask(0o022)
            yield Path(directory).resolve()
    finally:
        os.umask(original_umask)
