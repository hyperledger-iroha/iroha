# Intel AVX2 arithmetic execution

On 2026-10-22 the shared `iroha_fhe` arithmetic executed its x86-64 AVX2
instructions on an Intel Core i7-11800H under WSL2 (Microsoft hardware
virtualization, Linux `6.18.54.1-microsoft-standard-WSL2`). The runtime reports
AVX2 support. Rust was `1.93.1 (01f6ddf75 2026-02-11)`, host
`x86_64-unknown-linux-gnu`, LLVM 21.1.8.

This closes the C.2 instruction-execution gap for the checked kernels. It is
component correctness evidence: no named performance baseline, bare-metal
operating-system behavior, other accelerator or whole-node release is qualified.
The Parliament Xeon Silver 4310 requirement remains separate.

The checkout was based on `61bc20da4add8362651bf0f8dfbd2ce62388fb42` with existing
changes and the explicit qualification test. The 50-file inventory of FHE
Rust sources, fixtures and build manifests has SHA-256
`adab8489f8ce7bcf1b247eaf5ed7d6c001a461887a16071b40800d51e67206f4`.
Its digest is over sorted `relative-path + NUL + file-sha256 + newline` records.
The full inventory and toolchain identity are retained locally at
`dist/zk-delivery-handoff/review-c2/intel-avx2-evidence.json`; logs and generated
evidence remain untracked.

Each command below used the prefix
`scripts/cargo_fast.sh --target-slot intel-platform --stable-local-metadata --jobs 4 --`.

| Cargo command | Result |
| --- | --- |
| `test --locked -p iroha_fhe --lib accel:: -- --nocapture` | 10 passed before adding the explicit qualifier; includes the direct kernels and CPU-versus-lane-model comparison. No unavailable-AVX2 skip. |
| `test --locked -p iroha_fhe --lib accel::tests::required_avx2_execution -- --ignored --exact --nocapture` | 1 passed; requires x86-64 and selected AVX2 before executing every direct kernel parity check. |
| `test --locked -p iroha_fhe --lib --test canonical_vectors --test clearing --test rns_chain_vectors` | 164 unit, 9 canonical-vector, 7 clearing and 4 RNS-chain tests passed; the explicit qualifier is ignored in ordinary suites. |
| Same suite with `--no-default-features` | The same 184 tests passed with scalar production dispatch. |
| Explicit qualifier with `--no-default-features` | Expected rejection: `Scalar != Avx2`, Cargo exit 101. Scalar-only builds cannot qualify AVX2. |

Direct checks cover addition, subtraction, products, scalar products and cyclic
NTT, including modulus boundaries, zeroes, carries, non-lane-aligned tails and
transform lengths through 1,024. Canonical vectors also use independent
arbitrary-precision arithmetic. The explicit test is present on every target
and refuses the portable lane model before running the kernel checks.

Logs are `intel-avx2-initial.log`, `required-avx2.log`, `intel-simd.log`,
`intel-scalar.log` and `required-avx2-scalar-refusal.log` in the same evidence
directory. These runs emitted the existing unused macOS thermal mapper warning
from `iroha_measurement`; it did not affect arithmetic execution.

## IVM SHA-NI execution

The same host passed the existing explicit IVM production-path test:

```sh
scripts/cargo_fast.sh --target-slot intel-platform --stable-local-metadata --jobs 4 -- test --locked -p ivm --lib vector::sha256_cpu::tests::required_x86_sha_ni_executes_production_path_and_matches_scalar -- --ignored --exact --nocapture
```

Its isolated child emitted `IVM_NATIVE_SHA256_RECEIPT architecture=x86_64
completed_blocks=462`. The test counts actual native completions, compares
compression states and complete padded messages against scalar/independent
references, and checks that scalar and file-policy opt-outs perform no native
work. The passing test binary has SHA-256
`6a79336aff57c80a7a1c605a724bce2afe4df879d6a7bb2f7c2404f20472894c`.
The local source inventory and log are `dist/intel-platform/ivm-sha-ni-evidence.json`
and `ivm-required-sha-ni.log`. Existing unused-code warnings were retained;
this command was a hardware test, not a strict lint run.
