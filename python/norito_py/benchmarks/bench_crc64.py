# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Micro-benchmarks for the Norito CRC64 implementation.

Run from the repository root, for example:

```
python -m python.norito_py.benchmarks.bench_crc64 --size 8 --repeat 5
python -m python.norito_py.benchmarks.bench_crc64 --size 8 --repeat 5 --with-numba
```

The optional ``--with-numba`` flag attempts to import ``numpy`` and ``numba`` to
profile a JIT-accelerated variant.  If those dependencies are unavailable, the
script falls back to the baseline implementation and reports the reason.
"""

from __future__ import annotations

import argparse
import secrets
import statistics
import sys
import time
from pathlib import Path
from typing import Callable, Iterable, List, Optional, Tuple

def _load_crc64() -> tuple[int, list[int], Callable[[bytes, int], int]]:
    package_root = Path(__file__).resolve().parent.parent
    src_dir = package_root / "src"
    if str(src_dir) not in sys.path:
        sys.path.insert(0, str(src_dir))

    from norito.crc64 import _MASK, _TABLE, crc64  # type: ignore[attr-defined]

    return _MASK, _TABLE, crc64


_MASK, _TABLE, crc64 = _load_crc64()


def _random_payload(size_mib: float) -> bytes:
    byte_len = int(size_mib * 1024 * 1024)
    return secrets.token_bytes(byte_len)


def _time_callable(fn: Callable[[bytes], int], payload: bytes, repeat: int) -> Tuple[float, List[float]]:
    durations: List[float] = []
    for _ in range(repeat):
        start = time.perf_counter()
        fn(payload)
        durations.append(time.perf_counter() - start)
    return min(durations), durations


def _try_setup_numba() -> Optional[Callable[[bytes], int]]:
    try:
        import numpy as np  # type: ignore
        from numba import njit  # type: ignore
    except Exception as exc:  # pragma: no cover - optional dependency
        print(f"[bench] numba unavailable ({exc}); skipping accelerated benchmark")
        return None

    table = np.array(_TABLE, dtype=np.uint64)
    table.setflags(write=False)
    mask = np.uint64(_MASK)

    @njit(cache=True)  # type: ignore[misc]
    def _crc64_numba(view: np.ndarray, initial: int) -> int:
        crc = np.uint64(initial) & mask
        for byte in view:
            idx = ((crc >> np.uint64(56)) ^ np.uint64(byte)) & np.uint64(0xFF)
            crc = table[int(idx)] ^ ((crc << np.uint64(8)) & mask)
        return int(crc)

    def crc64_numba(payload: bytes, initial: int = 0) -> int:
        view = np.frombuffer(payload, dtype=np.uint8)
        return _crc64_numba(view, initial)

    return crc64_numba


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--size", type=float, default=8.0, help="payload size in MiB (default: 8)")
    parser.add_argument("--repeat", type=int, default=7, help="timing repetitions (default: 7)")
    parser.add_argument(
        "--with-numba",
        action="store_true",
        help="attempt to benchmark an optional NumPy/Numba implementation",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    payload = _random_payload(args.size)
    payload_mib = len(payload) / (1024 * 1024)

    benches: List[Tuple[str, Callable[[bytes], int]]] = [("pure_python", crc64)]
    if args.with_numba:
        numba_impl = _try_setup_numba()
        if numba_impl is not None:
            # Trigger compilation once so the benchmark measures steady-state performance.
            numba_impl(payload)
            benches.append(("numba", numba_impl))

    print(f"Benchmarking CRC64 on {payload_mib:.2f} MiB payload ({args.repeat} runs)")
    for name, func in benches:
        best, samples = _time_callable(func, payload, args.repeat)
        throughput = payload_mib / best if best > 0 else float("inf")
        mean = statistics.fmean(samples)
        print(
            f"{name:>12}: best={best * 1e3:7.3f} ms  mean={mean * 1e3:7.3f} ms  "
            f"throughput={throughput:8.2f} MiB/s"
        )

    return 0


if __name__ == "__main__":  # pragma: no cover - manual benchmark
    sys.exit(main())
