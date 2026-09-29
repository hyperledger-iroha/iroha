#!/usr/bin/env python3
"""Rebuild the IVM Metal library outside the shipping Cargo build.

The exact source order, target and language level are part of the artifact
identity. `--check` compares a fresh build byte-for-byte with the checked-in
library; `--output` writes a new candidate for separate review and signing.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
IVM = ROOT / "crates" / "ivm"
STEMS = (
    "metal_vadd32",
    "metal_vadd64",
    "metal_bitwise",
    "metal_sha256_compress",
    "metal_sha256_leaves",
    "metal_sha256_pairs_reduce",
    "metal_keccak_f1600",
    "metal_aes_rounds",
    "metal_ed25519",
)
TARGET = "air64-apple-macos11.0"
LANGUAGE = "macos-metal2.3"
BUNDLED = IVM / "metal" / "v1" / "ivm_kernels.metallib"


def source_path(stem: str) -> Path:
    """Resolve one of the fixed shader sources from its canonical stem."""
    if stem in {"metal_bitwise", "metal_ed25519"}:
        return IVM / "src" / f"{stem}.metal"
    return IVM / "src" / "assets" / "text_v1" / f"{stem}.metal"


def run(metal: Path, *, output: Path) -> bytes:
    """Compile all nine source units in order and link one Metal library."""
    with tempfile.TemporaryDirectory(prefix="ivm-metal-") as temp_dir:
        temp = Path(temp_dir)
        air_files: list[str] = []
        for stem in STEMS:
            source = source_path(stem).relative_to(ROOT)
            air = temp / f"{stem}.air"
            subprocess.run(
                [
                    str(metal),
                    "-target",
                    TARGET,
                    f"-std={LANGUAGE}",
                    "-c",
                    str(source),
                    "-o",
                    str(air),
                ],
                cwd=ROOT,
                check=True,
            )
            air_files.append(str(air))
        library = temp / "ivm_kernels.metallib"
        subprocess.run(
            [str(metal), "-target", TARGET, *air_files, "-o", str(library)],
            cwd=ROOT,
            check=True,
        )
        data = library.read_bytes()
    if output == BUNDLED:
        raise ValueError("use --check for the checked-in bundle")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(data)
    return data


def main() -> int:
    """Verify or emit a candidate, printing the compiler and artifact digest."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metal", type=Path, required=True, help="pinned Metal compiler")
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true")
    action.add_argument("--output", type=Path)
    args = parser.parse_args()
    compiler = args.metal.resolve(strict=True)
    version = subprocess.run(
        [str(compiler), "--version"], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout.splitlines()[0]
    if not version.startswith("Apple metal version 32023.921 "):
        parser.error(f"unqualified Metal compiler: {version}")
    with tempfile.TemporaryDirectory(prefix="ivm-metal-result-") as temp_dir:
        temp_output = Path(temp_dir) / "candidate.metallib"
        data = run(compiler, output=temp_output)
        if args.check:
            if data != BUNDLED.read_bytes():
                parser.error("rebuilt library differs from the checked-in bundle")
        else:
            output = args.output.resolve()
            if output == BUNDLED:
                parser.error("write a candidate outside the checked-in bundle for review")
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)
    print(version)
    print(f"sha256={hashlib.sha256(data).hexdigest()} bytes={len(data)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
