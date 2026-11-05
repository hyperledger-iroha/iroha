#!/usr/bin/env python3
"""Check the two co-built plugins' C exports and CUDA's portable-only ownership.

Read Cargo JSON artifact messages from a single build selecting both backend
packages. This checker does not build, discover substitute artifacts, or invoke
GPU work. See ../README.md for the exact build and verification commands.
"""

import argparse
import ctypes
import json
from pathlib import Path
import subprocess
import sys


def artifact_paths(messages: Path) -> dict[str, dict[str, Path]]:
    """Find the exact shared libraries and CUDA Rust archive from Cargo's output."""
    found: dict[str, dict[str, Path]] = {}
    for line in messages.read_text().splitlines():
        if not line.startswith("{"):
            continue
        record = json.loads(line)
        if record.get("reason") != "compiler-artifact":
            continue
        name = record["target"]["name"]
        if name not in {"gpuzstd_cuda", "gpuzstd_metal"}:
            continue
        if "cdylib" not in record["target"]["crate_types"]:
            continue
        artifacts = found.setdefault(name, {})
        for filename in record["filenames"]:
            path = Path(filename).resolve(strict=True)
            if path.suffix in {".dylib", ".so", ".dll"}:
                kind = "plugin"
            elif path.suffix == ".rlib":
                kind = "rlib"
            else:
                continue
            previous = artifacts.setdefault(kind, path)
            if previous != path:
                raise AssertionError(f"ambiguous {name} {kind}: {previous}, {path}")
    for name in ("gpuzstd_cuda", "gpuzstd_metal"):
        if "plugin" not in found.get(name, {}):
            raise AssertionError(f"Cargo output lacks co-built {name} plugin")
    if "rlib" not in found["gpuzstd_cuda"]:
        raise AssertionError("Cargo output lacks CUDA Rust archive")
    return found


def output(*command: str) -> str:
    """Read a native artifact inspection tool's successful output."""
    return subprocess.run(command, check=True, capture_output=True, text=True).stdout


def inspect_symbols(artifacts: dict[str, dict[str, Path]]) -> None:
    """Reject Metal implementation imports in CUDA and missing plugin exports."""
    if sys.platform not in {"darwin", "linux"}:
        raise RuntimeError("artifact symbol inspection currently supports macOS and Linux")
    for name, paths in artifacts.items():
        plugin = str(paths["plugin"])
        arguments = ("-gU",) if sys.platform == "darwin" else ("-D", "--defined-only")
        symbols = output("nm", *arguments, plugin)
        exported = [line.split()[-1].removeprefix("_") for line in symbols.splitlines() if line.split()]
        for symbol in c_abi_symbols(name):
            if exported.count(symbol) != 1:
                raise AssertionError(f"{name} must export exactly one {symbol}")
    cuda = artifacts["gpuzstd_cuda"]
    # The archive retains cross-crate references even when a linker discards a
    # conflicting definition from the final dylib. This catches the original
    # CUDA -> Metal codec edge without relying on a linker's warning policy.
    for path in (cuda["rlib"], cuda["plugin"]):
        if "gpuzstd_metal" in output("nm", "-a", str(path)):
            raise AssertionError(f"CUDA artifact imports Metal implementation symbols: {path}")
    if sys.platform == "darwin":
        if "Metal.framework" in output("otool", "-L", str(cuda["plugin"])):
            raise AssertionError("CUDA plugin imports the Metal framework")


def c_abi_symbols(backend: str) -> tuple[str, str]:
    """Select the exact current exports owned by one known backend."""
    if backend == "gpuzstd_cuda":
        return "gpuzstd_cuda_compress", "gpuzstd_cuda_decompress"
    if backend == "gpuzstd_metal":
        return "gpu_zstd_compress", "gpu_zstd_decompress"
    raise ValueError(f"unknown GPU Zstd backend: {backend}")


def verify_c_abi(backend: str, plugin: Path, require_cuda_unavailable: bool) -> None:
    """Exercise actual exported functions without requiring an accelerator."""
    library = ctypes.CDLL(str(plugin), mode=ctypes.RTLD_LOCAL)
    pointer = ctypes.c_void_p
    size_pointer = ctypes.POINTER(ctypes.c_size_t)
    compress_symbol, decompress_symbol = c_abi_symbols(backend)
    compress = getattr(library, compress_symbol)
    compress.argtypes = [pointer, ctypes.c_size_t, ctypes.c_int, pointer, size_pointer]
    compress.restype = ctypes.c_int
    decompress = getattr(library, decompress_symbol)
    decompress.argtypes = [pointer, ctypes.c_size_t, pointer, size_pointer]
    decompress.restype = ctypes.c_int

    destination = ctypes.create_string_buffer(64)
    length = ctypes.c_size_t(len(destination))
    if compress(None, 0, 1, destination, ctypes.byref(length)) != 1:
        raise AssertionError(f"{plugin}: compression must reject null input")
    if decompress(None, 0, destination, ctypes.byref(length)) != 1:
        raise AssertionError(f"{plugin}: decompression must reject null input")

    # One canonical raw Zstd block containing four bytes, independent of the
    # shared encoder: magic, single-segment/FCS header, final raw block, payload.
    frame = ctypes.create_string_buffer(bytes.fromhex("28b52ffd200421000074657374"))
    length.value = len(destination)
    if decompress(frame, 13, destination, ctypes.byref(length)) != 0:
        raise AssertionError(f"{plugin}: raw frame decode failed")
    if length.value != 4 or destination.raw[:4] != b"test":
        raise AssertionError(f"{plugin}: raw frame decoded incorrectly")
    if require_cuda_unavailable:
        payload = ctypes.create_string_buffer(b"CUDA backend identity")
        length.value = len(destination)
        if compress(payload, len(payload) - 1, 1, destination, ctypes.byref(length)) != 3:
            raise AssertionError("CUDA plugin did not report unavailable without CUDA kernels")


def main() -> None:
    """Validate co-built backend artifacts supplied by one Cargo invocation."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cargo_messages", type=Path)
    parser.add_argument("--cuda-unavailable", action="store_true")
    args = parser.parse_args()
    artifacts = artifact_paths(args.cargo_messages)
    inspect_symbols(artifacts)
    for name, paths in artifacts.items():
        verify_c_abi(name, paths["plugin"], args.cuda_unavailable and name == "gpuzstd_cuda")
    print("Both plugin C ABIs passed; CUDA artifacts contain no Metal implementation.")


if __name__ == "__main__":
    main()
