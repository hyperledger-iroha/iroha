# Iroha native Python boundary

`iroha-native` owns the packaged `iroha_native._crypto` Rust extension and its
strict loader. Both the full SDK and Torii account operations use this one
cryptographic and canonical identity authority. ABI 24, all eleven account key
algorithms, and full multisig policies are mandatory. Missing native artifacts
fail explicitly; there is no alternate module, structural validator, or build
output search path. The loader creates the extension from its inspected packaged
filesystem spec and rejects pre-seeded or replaced modules; callers must obtain
the native module through `load_crypto_extension()`.

Build this wheel with `IROHA_PYTHON_SKIP_RUNTIME_LINK=1 maturin build --release`
in this directory using the repository toolchain, then install it alongside the
pure `iroha-python` wheel.
The build setting omits the CPython shared-library link used by Rust test
executables; packaged extensions resolve Python symbols from their interpreter.
The Rust crate remains at `../iroha_python/iroha_python_rs`; no transport package
is imported by this native boundary.
