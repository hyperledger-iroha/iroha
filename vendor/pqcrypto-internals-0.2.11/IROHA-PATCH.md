# Iroha portability patch

Source: the crates.io `pqcrypto-internals` 0.2.11 release. The upstream
`Cargo.toml.orig` records its original package definition.

The FFI uses `core::ffi::c_int` and `usize` for C `int` and `size_t`, so it does
not depend on the OS bindings exposed by `libc`. This includes the browser
`wasm32-unknown-unknown` target, where `libc` has no such exports. The Rust/C
pointer and size ABI is checked by the SDK's maintained C toolchain preflight.

The original `getrandom::fill` call and failure behavior are unchanged. Browser
entropy is the real `getrandom` 0.3 `wasm_js` backend selected by
`iroha_js_codec_wasm`; native entropy backends remain unchanged. All upstream C
sources, headers and build selection are preserved byte for byte. The unused
OS-specific `randombytes.c` remains uncompiled, as in the upstream build.

The two added unit tests exercise the exported ABI with real entropy and check
exact buffer bounds and a zero-length request. Run them in the existing SDK
validation lane with `cargo test --locked --release -p pqcrypto-internals --lib`.
