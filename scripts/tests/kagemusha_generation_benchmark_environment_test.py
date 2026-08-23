"""Security regressions for the isolated Kagemusha generation benchmark."""

from __future__ import annotations

import os
from pathlib import Path
from unittest import mock

from scripts import run_kagemusha_v4_generation_benchmark as benchmark


def test_benchmark_child_environment_drops_ambient_execution_controls() -> None:
    scratch = Path("/private/benchmark-scratch")
    hostile = {
        "DYLD_INSERT_LIBRARIES": "/tmp/hostile.dylib",
        "LD_PRELOAD": "/tmp/hostile.so",
        "MALLOC_CONF": "prof:true",
        "RUSTFLAGS": "-C target-cpu=native",
        "RUST_MIN_STACK": "1",
        "TMP": "/tmp/ambient",
    }
    with mock.patch.dict(os.environ, hostile, clear=False):
        environment = benchmark._benchmark_child_environment(scratch)

    assert environment == {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": benchmark.candidate_guard.FIXED_CANDIDATE_CHILD_PATH,
        "TMPDIR": os.fspath(scratch),
    }
    assert hostile.keys().isdisjoint(environment)
