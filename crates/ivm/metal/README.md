# IVM Metal V1 artifact

`v1/ivm_kernels.metallib` is the single embedded macOS Metal library. Ordinary
builds and process startup load these bytes; they do not compile source or
download a shader. The build and runtime both check SHA-256
`fe62a035ab7481646e64c2f272f38669c353b95f5ab79ec5876d412f88237170`.
The nine fixed-order source units have framed SHA-256
`9c8f09f49746b6909d1565b6bc3175bb77e86c796bf911c07a722bd540218de5`.

The candidate was built with Apple Metal
`32023.921 (metalfe-32023.921.6)`, target `air64-apple-macos11.0`, and
`macos-metal2.3`. On a host with that optional Xcode Metal toolchain, rebuild
and compare it with:

```sh
python3 scripts/build_ivm_metal_bundle.py --metal /absolute/path/to/metal --check
```

The script compiles all source units in the fixed order and checks the exact
172,793-byte output. The source inventory is also checked by the build script.
Metal availability and per-kernel parity remain runtime device qualifications;
the CPU path remains the deterministic fallback.

TODO: Bind this exact candidate and its compiler/SDK identity to a reviewed
release-signing key and signed provenance manifest, then qualify the unchanged
release binary on every supported macOS/GPU/driver combination. The local
embedded digest and one-host byte-for-byte rebuild are not signed provenance.
