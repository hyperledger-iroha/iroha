#!/usr/bin/env python3
"""Build the package-owned browser codec in an explicitly selected warm lane.

No tools are installed. The caller supplies a pinned WASI SDK for C headers and
Clang; Rust still targets wasm32-unknown-unknown and links its own real memory
intrinsics, not WASI libc. This command never consumes runtime credentials.
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib


PACKAGE = Path(__file__).resolve().parents[1]
ROOT = PACKAGE.parents[1]
TARGET = "wasm32-unknown-unknown"
CRATE = "iroha_js_codec_wasm"
RUST_VERSION = "1.93.1"
BINDGEN_VERSION = "0.2.122"
OUTPUT_NAMES = {
    f"{CRATE}.js", f"{CRATE}.d.ts", f"{CRATE}_bg.wasm", f"{CRATE}_bg.wasm.d.ts"
}


def run(command, *, env=None, input_text=None):
    """Execute an argument vector without a shell and fail on the first error."""
    subprocess.run(
        [str(part) for part in command], cwd=ROOT, env=env, input=input_text,
        text=True, check=True,
    )


def capture(command, *, env=None):
    return subprocess.check_output(
        [str(part) for part in command], cwd=ROOT, env=env, text=True,
    ).strip()


def absolute_path(value):
    path = Path(value).expanduser()
    if not path.is_absolute():
        raise argparse.ArgumentTypeError("use an absolute path")
    return path


def tool(path):
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError(f"required executable is unavailable: {path}")
    return path


def c_toolchain(sdk):
    clang = tool(sdk / "bin/clang")
    ar = tool(sdk / "bin/llvm-ar")
    sysroot = sdk / "share/wasi-sysroot"
    # Recent SDKs use a target-specific include directory; earlier SDKs use
    # include/. Select only a complete standard header set within this SDK.
    candidates = [sysroot / "include/wasm32-wasip1", sysroot / "include/wasm32-wasi",
                  sysroot / "include"]
    include = next((path for path in candidates if all(
        (path / name).is_file() for name in ["stdlib.h", "string.h", "stdint.h"]
    )), None)
    if include is None:
        raise ValueError("the selected WASI SDK has no complete C header directory")
    # cc-rs reads this space-delimited variable. Reject paths requiring a second
    # quoting language instead of accidentally splitting or reinterpreting them.
    if any(character.isspace() or character in "\"'\\" for character in str(sysroot)):
        raise ValueError("the WASI SDK path must not contain whitespace or quotes")
    # SDK 34's stdint.h includes wasi/version.h even when only integer types
    # are requested. Admit those C declarations explicitly. This affects C
    # preprocessing only: the target remains wasm32-unknown-unknown, no WASI
    # libc is linked, and the locked PQClean/blst/blake3/zstd C sources contain
    # no __wasi__/__wasip* implementation branches. Rust supplies memcpy/memset.
    flags = [f"--sysroot={sysroot}", "-isystem", str(include), "-D__wasi__=1", "-ffreestanding"]
    return clang, ar, flags


def rust_toolchain(sysroot):
    """Verify one standalone Rust distribution, without depending on rustup."""
    sysroot = sysroot.resolve(strict=True)
    rustc = tool(sysroot / "bin/rustc").resolve(strict=True)
    cargo = tool(sysroot / "bin/cargo").resolve(strict=True)
    if rustc.parent != sysroot / "bin" or cargo.parent != sysroot / "bin":
        raise ValueError("rustc and cargo must belong to the selected Rust sysroot")
    if f"release: {RUST_VERSION}" not in capture([rustc, "--version", "--verbose"]).splitlines():
        raise ValueError(f"Rust {RUST_VERSION} is required")
    if capture([cargo, "--version"]).split()[:2] != ["cargo", RUST_VERSION]:
        raise ValueError(f"Cargo {RUST_VERSION} is required")
    if Path(capture([rustc, "--print", "sysroot"])).resolve() != sysroot:
        raise ValueError("rustc reports a different sysroot from the selected toolchain")
    libraries = Path(capture([rustc, "--print", "target-libdir", "--target", TARGET]))
    expected = sysroot / "lib/rustlib" / TARGET / "lib"
    if libraries.resolve() != expected or not all(
        list(libraries.glob(f"lib{crate}-*.rlib")) for crate in ["std", "core"]
    ):
        raise ValueError(f"the selected Rust sysroot needs its {TARGET} std/core libraries")
    return rustc, cargo


def build_environment(target_dir, clang, ar, flags):
    env = os.environ.copy()
    # Avoid inherited native CPU/linker flags or generic C flags changing the
    # reviewed browser ABI. Platform flags below apply only to the Wasm target.
    for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CFLAGS", "TARGET_CFLAGS",
                 "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS"]:
        if env.get(name):
            raise ValueError(f"unset {name} before building the browser codec")
    env.update({
        "CARGO_TARGET_DIR": str(target_dir),
        "CC_wasm32_unknown_unknown": str(clang),
        "CC_wasm32-unknown-unknown": str(clang),
        "AR_wasm32_unknown_unknown": str(ar),
        "AR_wasm32-unknown-unknown": str(ar),
        # cc-rs concatenates both CFLAGS spellings rather than selecting one.
        # Clear the lower-priority spelling so each reviewed flag occurs once.
        "CFLAGS_wasm32_unknown_unknown": "",
        "CFLAGS_wasm32-unknown-unknown": " ".join(flags),
    })
    return env


def preflight(args):
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    versions = {package["version"] for package in lock["package"]
                if package["name"] == "wasm-bindgen"}
    if versions != {BINDGEN_VERSION}:
        raise ValueError("Cargo.lock and the supported wasm-bindgen CLI version differ")
    configured_rust = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())
    if configured_rust["toolchain"]["channel"] != RUST_VERSION:
        raise ValueError("the browser build's Rust version must match rust-toolchain.toml")
    bindgen = tool(args.wasm_bindgen)
    if capture([bindgen, "--version"]) != f"wasm-bindgen {BINDGEN_VERSION}":
        raise ValueError(f"wasm-bindgen CLI {BINDGEN_VERSION} is required")
    rustc, cargo = rust_toolchain(args.rust_toolchain)
    node = tool(args.node)
    clang, ar, flags = c_toolchain(args.wasi_sdk)
    env = build_environment(args.target_dir, clang, ar, flags)
    env["RUSTC"] = str(rustc)
    env["PATH"] = str(rustc.parent) + os.pathsep + env.get("PATH", "")
    if "wasm32" not in capture([clang, "--print-targets"]):
        raise ValueError("the selected Clang has no wasm32 backend")
    run([clang, f"--target={TARGET}", *flags, "-Werror", "-fsyntax-only", "-x", "c", "-"],
        env=env, input_text="""#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(void *) == 4, "browser pointers must be 32 bit");
_Static_assert(sizeof(size_t) == 4, "browser size_t must be 32 bit");
_Static_assert(sizeof(uint64_t) == 8, "browser uint64_t must be 64 bit");
void memory_check(uint8_t *dst, const uint8_t *src, size_t n) {
    memcpy(dst, src, n);
    memset(dst, 0, n);
}
""")
    run([node, "-e", "new WebAssembly.Module(new Uint8Array([0,97,115,109,1,0,0,0]));"])
    # This resolves features without compiling and retains the multicore guard.
    features = capture([cargo, "tree", "--locked", "--offline",
                        "--target", TARGET, "-p", CRATE, "-e", "normal,build,features",
                        "-i", "halo2-axiom"], env=env)
    if 'halo2-axiom feature "multicore"' in features:
        raise ValueError("browser feature graph unexpectedly enables Halo2 multicore")
    return env, cargo, node


def inspect_imports(wasm, node):
    script = """const fs = require('node:fs');
const module = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
process.stdout.write(JSON.stringify(WebAssembly.Module.imports(module)));
"""
    imports = json.loads(capture([node, "-e", script, wasm]))
    # wasm-bindgen 0.2.122's web generator rewrites global/method imports to
    # this exact self-module name (cli-support/src/js/mod.rs generate_imports).
    allowed = {f"./{CRATE}_bg.js"}
    if any(item["module"] not in allowed for item in imports):
        raise ValueError("generated browser Wasm imports an unsupported host module")


def publish(staging, output, node):
    # The consumer copies under build-dist's lock. Use that same lock owner for
    # publication, so its copy cannot combine two generated codec generations.
    run([node, PACKAGE / "scripts/publish-browser-codec.mjs",
         "--staging", staging, "--output", output])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust-toolchain", required=True, type=absolute_path,
                        help="one standalone Rust 1.93.1 sysroot containing bin/rustc and bin/cargo")
    parser.add_argument("--wasi-sdk", required=True, type=absolute_path)
    parser.add_argument("--wasm-bindgen", required=True, type=absolute_path)
    parser.add_argument("--node", required=True, type=absolute_path)
    parser.add_argument("--target-dir", required=True, type=absolute_path,
                        help="one persistent browser build lane, separate from native Cargo output")
    parser.add_argument("--output-dir", type=absolute_path, default=PACKAGE / "wasm")
    parser.add_argument("--preflight-only", action="store_true")
    args = parser.parse_args()
    env, cargo, node = preflight(args)
    if args.preflight_only:
        print("browser codec toolchain and feature preflight passed")
        return
    args.output_dir.parent.mkdir(parents=True, exist_ok=True)
    # Serialize generation/publication, and retain Cargo's warm target directory.
    with (args.output_dir.parent / ".build-wasm.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        run([cargo, "build", "--locked", "--release",
             "--target", TARGET, "-p", CRATE], env=env)
        with tempfile.TemporaryDirectory(prefix=".wasm-stage-", dir=args.output_dir.parent) as temporary:
            staging = Path(temporary) / "wasm"
            run([args.wasm_bindgen, "--target", "web", "--out-name", CRATE,
                 "--out-dir", staging, args.target_dir / TARGET / "release" / f"{CRATE}.wasm"])
            if set(path.name for path in staging.iterdir()) != OUTPUT_NAMES:
                raise ValueError("wasm-bindgen emitted an unexpected artifact set")
            inspect_imports(staging / f"{CRATE}_bg.wasm", node)
            publish(staging, args.output_dir, node)
    print(f"browser codec generated at {args.output_dir}; run the SDK browser conformance tests")


if __name__ == "__main__":
    main()
