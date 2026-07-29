# IVM CUDA PTX artifacts

The CUDA source kernels in this directory are optional acceleration paths. Their
outputs must remain bit-identical to the scalar implementation; the PTX build
path must never silently substitute a placeholder.

`IVM_CUDA_PTX_MODE` selects the build-time artifact policy:

- `bundled` (default) copies the checked-in `cuda/<kernel>.ptx` bytes into
  Cargo's output directory. Missing or structurally invalid PTX fails the
  build.
- `generate` explicitly invokes `nvcc` for every `.cu` source and writes PTX
  only into Cargo's output directory. This mode is for qualification runners,
  not ordinary or release builds.
- `check` invokes `nvcc`, validates both outputs, requires every generated file
  to be byte-identical to its checked-in counterpart, and then installs the
  checked-in bytes.

Generation and checking use `IVM_CUDA_NVCC` (or `NVCC`),
`IVM_CUDA_GENCODE`, and `IVM_CUDA_NVCC_EXTRA`. The default generation target is
`arch=compute_86,code=sm_86`, matching the current CUDA hardware lane. A release
artifact still requires a pinned CUDA image and toolchain; the default alone is
not provenance.

Examples:

```sh
IVM_CUDA_PTX_MODE=generate cargo build -p ivm --features cuda
IVM_CUDA_PTX_MODE=check cargo build -p ivm --features cuda
```

## Release blocker

TODO: reproducibly generate and check in all 11 real artifacts:
`add.ptx`, `aes.ptx`, `bitonic_sort.ptx`, `bn254.ptx`, `poseidon.ptx`,
`sha256.ptx`, `sha256_leaves.ptx`, `sha256_pairs_reduce.ptx`, `sha3.ptx`,
`signature.ptx`, and `vector.ptx`.

TODO: attach a signed manifest binding each PTX digest to its `.cu` source
digest, the pinned CUDA image digest, exact `nvcc` version and flags, target
profile, and two clean byte-identical generation runs. Until both TODOs are
closed, the default `bundled` CUDA build intentionally fails closed.

TODO: remove `add.cu`/`vector_add_f32` or bind it to an explicit
non-consensus diagnostic artifact class before signing that manifest. All
consensus-facing kernels still require real-hardware scalar parity, malformed
input, and failure-path qualification.
