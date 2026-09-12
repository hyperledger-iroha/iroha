# Iroha portability patch

Source: the crates.io `pqcrypto-mldsa` 0.1.2 release. The upstream
`Cargo.toml.orig` records its original package definition.

The only Rust implementation change replaces `libc::c_int` with
`core::ffi::c_int`; the unnecessary `libc` dependency is removed. All C sources
and build logic are unchanged. The vendored PQClean tree includes its common
sources and all ML-DSA-44/65/87 clean, AVX2 and AArch64 implementations, with their
original license files. Other algorithms shipped in the upstream archive but
never selected by this package's build script are omitted.

Entropy still comes from `pqcrypto-internals` through its real `getrandom`
backend. Existing `ffi::test_mldsa{44,65,87}_clean::test_ffi` tests exercise the
actual C key generation, signing and verification ABI. Run them in the existing
SDK validation lane; do not substitute crypto or entropy implementations.
