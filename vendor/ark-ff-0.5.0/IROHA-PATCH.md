# Iroha dependency patch

Upstream: ark-ff 0.5.0, crates.io archive SHA-256
`a177aba0ed1e0fbb62aa9f6d0502e9b46dad8c2eab04c14258a1212d2557ea70`.

Replace the unmaintained paste procedural macro with maintained pastey 0.2.3. The dependency keeps its upstream import name; runtime arithmetic and algorithms are unchanged.

The crate always links Rust `std`. The retired no-std crate attribute and
WebAssembly-specific multiplication/unroll paths are removed; native widening
multiplication and the native 12-limb unroll policy remain unchanged.
`tests/native_arithmetic.rs` locks full-width product vectors and canonical
Montgomery reduction to independent fixed integer expectations. Copy this
directory outside the repository and run
`cargo test --manifest-path <copy>/Cargo.toml --test native_arithmetic`, then
repeat with `--features std`. This keeps the standalone test lockfile outside
the reviewed vendor tree. Downstream standalone curve suites also need a Cargo
`patch.crates-io.ark-ff.path` override pointing to this patched source.

The normalized Cargo.toml is the active manifest; Cargo.toml.orig records the
unmodified upstream manifest. Remove the macro dependency patch when an upstream
release carries it; retain the repository's native-only build policy.
