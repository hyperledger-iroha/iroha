# Browser codec adapter

Six wasm-bindgen exports delegate account admission and instruction encoding to
`iroha_js_codec`. Account results cross the boundary as Norito JSON strings;
instruction frames and compact archives use byte arrays. The JavaScript SDK
owns initialization, strict result adaptation and immutable binding publication.
There is no signing, networking, filesystem or runtime credential interface.

The browser target uses the standard library, real WebCrypto entropy providers,
PQClean and BLS implementations, and serial Halo2 execution. Native daemon
features and scheduling remain unchanged.

Build with Python 3.11 or newer using one persistent browser Cargo lane and an
explicit, pinned LLVM/WASI SDK plus the exact wasm-bindgen CLI from Cargo.lock:

```sh
python3 javascript/iroha_js/scripts/build-wasm.py \
  --rust-toolchain /absolute/path/to/rust-1.93.1-sysroot \
  --wasi-sdk /absolute/path/to/wasi-sdk \
  --wasm-bindgen /absolute/path/to/wasm-bindgen \
  --node /absolute/path/to/node \
  --target-dir /absolute/path/to/persistent-browser-cargo-lane \
  --preflight-only
```

Remove `--preflight-only` to build. The script never installs tools or clears
Cargo output. It checks one standalone Rust sysroot and its browser std/core
libraries, CLI version, C backend/headers and Halo2
features before compilation, then rejects unresolved OS/WASI imports before
publishing the generated files to `javascript/iroha_js/wasm/` under the SDK's
existing distribution lock. WASI SDK headers
are used only to compile the freestanding C dependencies: the output remains
`wasm32-unknown-unknown`, using Rust's real compiler memory intrinsics.
SDK 34 requires the C-only `__wasi__` declaration macro even for its integer
types. The script supplies it only to the C target flags; the locked crypto C
sources have no implementation branches controlled by that macro. No Rust WASI
configuration, OS import or WASI libc link is introduced. Preflight checks the
32-bit pointer/size ABI before compilation.

Run the shared Rust codec tests, adapter admission tests, serial MSM regressions,
offline `scripts/test_build_wasm.py`, and the SDK's real browser conformance tests
before publishing a package. A successful Wasm link alone is not browser proof.
