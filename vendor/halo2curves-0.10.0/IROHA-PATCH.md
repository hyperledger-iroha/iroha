# Iroha dependency patch

Upstream: halo2curves 0.10.0, crates.io archive SHA-256
`d124fcb02bccc7a9253a8aaa99a365f47f0f8b5a363b26a494f06f4a221bd4b7`.

Replace the unmaintained paste procedural macro with maintained pastey 0.2.3. The dependency keeps its upstream import name; runtime arithmetic and algorithms are unchanged.

The crate always links Rust `std`. The retired no-std crate attribute,
no-std/WebAssembly CI jobs and active WebAssembly-only getrandom test dependency
are removed. The existing `std` feature still selects parallel algorithms and
I/O helpers; disabling it selects native serial behavior, not another runtime.

The normalized Cargo.toml is the active manifest; Cargo.toml.orig records the
unmodified upstream manifest. Remove the macro dependency patch when an upstream
release carries it; retain the repository's native-only build policy.
