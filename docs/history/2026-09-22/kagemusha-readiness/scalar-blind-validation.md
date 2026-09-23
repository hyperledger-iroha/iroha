# KAGEMUSHA scalar and blind component validation — 2026-09-22

The same 27 selected Axiom library tests passed in both default and no-multicore
features, with zero failed or ignored tests: 54 executions, 27 distinct tests. This is scoped component evidence. It does not qualify a
complete stored proof, Core/SDK integration, release artifacts, hardware,
whole-process memory/latency, or independent cryptographic review. The next
multiopening continuation through P and the subsequent verifier x3-coincidence
repair are outside this tested candidate.

| Selection | Passing tests |
| --- | ---: |
| Scalar transcript, opening sources/H, and source/transcript fault cleanup | 4 |
| Advice retirement/quotient bridge, including two new original-blind folding tests | 12 |
| Original quotient commitment and ChallengeX differential | 1 |
| Selector metadata and retained seeded-proof checks | 5 |
| Indexed structured-key scanner | 5 |
| Total per feature mode | 27 |

The scalar tests compare ordinary scalar order/bytes, opening source coefficients,
original blinds and lazy H reconstruction in both Pasta fields. They also cover
budget/refusal paths, partial-output erasure, read/transcript errors, unwind and
late identity changes. Blind tests compare original Horner contributions while
preserving phase allocations, challenges, context and cursor; failure injection
covers each pre/post receipt-validation call. Selector seeded-proof checks are
small independent fixtures, not complete stored KAGEMUSHA proofs.

## Execution boundary

The runner used the existing warm vendor target and this command from the
canonical `optimizations` checkout:

```sh
cargo iroha-fast -- test \
  --manifest-path /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/Cargo.toml \
  --locked --offline \
  --target-dir /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/target \
  --config 'patch.crates-io.halo2curves-axiom.path="/Users/takemiyamakoto/dev/iroha/vendor/halo2curves-axiom"' \
  --lib --no-run --message-format=json
```

Cargo selected the freshly rebuilt `halo2_axiom-659f6a152d822c07` test executable
with `batch,circuit-params,default,multicore`, debug assertions and overflow checks,
optimization level 0. Compilation succeeded in 29.258 seconds. The runner listed
and executed the exact five selections above from that artifact. This timing is
compilation time, not proof latency. Context HEAD was
`1b8e5f92b6dadb1dcdd16f945286560e31b347ff`; the source maps also capture uncommitted
candidate bytes, so HEAD alone does not identify the tested implementation.

The no-multicore build used the same command plus
`--no-default-features --features batch,circuit-params`; it produced
`halo2_axiom-cf047cd5e9d47e25` in 32.441 seconds. All five selections passed again.
Each run's 248-entry before/after source maps match, and both runs use the same
source map. Each executable hash remained unchanged through its execution.
Both results explicitly record `production_qualified: false`.

| Evidence under `target/kagemusha-validation/20260922/combined-default-r2/` | SHA-256 |
| --- | --- |
| `result.json` | `01eb6aefd4eb9cc85aaee455f7a06d66ed5bfc6cdd471ad351fd677a8c3fe8d1` |
| `sources-before.json` and `sources-after.json` | `f02ed6c723c13cdcbdda1e3df52cd3b730f924618785507cd8c8142fc9376042` |
| `compile.json` | `6a9f09dddc98211d24e4bdb958252c126b43012fe80ae68cc55fbf7ad7ec8c18` |
| `artifact.json` | `1467121066255f79611e3bf0fd8f86819d561b39ec5543b6a46e771718479ddd` |
| Selected executable | `80ea0e809aa5441f2e5309dcc8628ee66a3da596243c4d4b0f547f6965094237` |

| Evidence under `target/kagemusha-validation/20260922/combined-no-multicore-r1/` | SHA-256 |
| --- | --- |
| `result.json` | `05a8d36062cd873358bdc9d5c8433cbf45d6a07e8d0ccd9f26da4ef842feb3ae` |
| `sources-before.json` and `sources-after.json` | `f02ed6c723c13cdcbdda1e3df52cd3b730f924618785507cd8c8142fc9376042` |
| `compile.json` | `cf59c0da89042211ba41be9183acb8f0145d703daf42f8d7e6e977e00e3daa4a` |
| `artifact.json` | `9d10551e05415486b0b451a68c4b0e56f87994aac6c34961f80e2e1b3f273560` |
| Selected executable | `1860f9da18d725179a179cfa43afe54b532a246636f62785396bf3b3c02687bf` |

Both local directories also contain the exact group names, compiler JSON, test
stdout/stderr and per-command durations. Older missing target receipts remain
recorded as historical claims in the adjacent archive; these fresh results do
not reconstruct them.
