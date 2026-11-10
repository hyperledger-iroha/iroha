# GPU Zstd backend ownership

The CUDA and Metal plugins each retain the `gpu_zstd_compress` and
`gpu_zstd_decompress` C exports in their separate shared libraries. Both use
`gpuzstd_codec` for portable frame coding and the sequence layout. Neither
backend depends on the other's device implementation or C exports. Metal's
existing Rust codec modules and sequence type remain available as re-exports.

Build both packages in one Cargo invocation to exercise workspace feature
unification. Do not test this by disabling Metal's default `c-abi` feature:

```sh
cargo iroha-fast -- test --locked -p gpuzstd_codec -p gpuzstd_metal -p gpuzstd_cuda
cargo iroha-fast -- build --locked -p gpuzstd_metal -p gpuzstd_cuda --message-format=json > /tmp/gpuzstd-artifacts.json
python3 crates/gpuzstd_cuda/tests/verify_workspace_artifacts.py /tmp/gpuzstd-artifacts.json
```

On a host built without CUDA kernels, append `--cuda-unavailable` to the Python
command. The checker uses the exact artifacts reported by Cargo, verifies both
plugins' exports and raw-frame decoding, and rejects Metal symbols or framework
imports in the CUDA artifact. Its archive check catches the original dependency
edge even when the linker discards a duplicate definition. Symbol inspection
currently supports macOS and Linux and requires their native `nm` (plus `otool`
on macOS). The plugins intentionally share C symbol names for separate loading;
linking both C-exporting Rust libraries into one executable is not supported.

Existing portable codec tests now run under `gpuzstd_codec`. Metal's hardware
parity tests consume that crate's `test-fixtures` CPU references. Real-device
qualification remains the separate check described in
[`gpuzstd_cuda_pipeline.md`](../../specs/gpuzstd_cuda_pipeline.md).
