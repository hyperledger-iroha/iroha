# Bounded indexed-key reader: scoped validation — 2026-09-22

The crate-private indexed reader copies native-basis intervals from the original
structured key frame using constant-size decoding scratch: mask coefficients,
constant/bitset/raw fixed Lagrange values and permutation Lagrange values recovered
from target IDs. It allocates no full column or transform. Invalid requests,
decoding or I/O failure, and unwind clear the entire destination. Valid empty
intervals perform no I/O.

Original-reader authentication, freshness and failure poisoning belong to the
consuming owner. This parser neither binds replacement bytes to an authenticated
index nor admits a proof. Stored proving and normal Core generation still retain
dense keys; consuming indexed-owner integration remains open.

## Completed component checks

Work and receipts are confined to `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. Receipt root: `target/kagemusha-validation/20260922/`.

| Window | Result | Compile / execution time |
| --- | --- | ---: |
| `indexed-reader-default-r1` | 18 passed, zero failed or ignored | 26.027 s / 8.381 s |
| `indexed-reader-no-multicore-r1` | Same 18 passed, zero failed or ignored | 22.544 s / 10.391 s |
| `indexed-reader-production-check-r1` | Non-test library check passed | 0.311 s, cached artifact fresh |

The same 18 distinct functions execute twice: **36 executions**. Four reader tests
cover both Pasta fields against the original dense decoder, bounded source reads,
short I/O, invalid columns/ranges/overflow, noncanonical scalars/targets, read/seek
errors, EOF, wrong seek positions and unwind. Fourteen existing structured-codec
functions cover scanner, canonical encoding, consuming writes and dense reloads.
These are component fixtures; dense reload proof checks do not connect the new
reader to stored proving or qualify KAGEMUSHA monetary proofs or device memory.

The original 18-function selection does not cover consecutive permutation targets
crossing n−1→n in the optimized label path. That additional regression is staged
under a shared vendor-source hold, not installed or qualified by these receipts.

All six before/after maps across the three windows match byte-for-byte: 262
captured source/build inputs. Each test executable is unchanged during its window.
Compared with the [preceding guarded-inner candidate](inner-validation.md), exactly
four captured paths differ under `vendor/halo2-axiom/src/plonk/structured_key/`:
`indexed.rs`, `indexed/reads.rs`, `indexed_reads_tests.rs` and `indexed_tests.rs`.
The earlier 54 inner-test executions remain evidence for that earlier candidate.

## Receipts and commands

| Artifact (under the receipt root) | SHA-256 |
| --- | --- |
| All three before/after source maps | `92cab3fb85e08a5df6ab20f2473367294b07117ed8d65cb562fd58a6a6e0cefe` |
| `indexed-reader-default-r1/result.json` | `d2da36f78e149a6cc0392adea1a9378e5f945ab996515ab775925a3e3104c91b` |
| Default executable | `711d2d52c45d9f02eaca555c0dd5d3b9293a07ae58d1db5a94463a8445cb6cef` |
| `indexed-reader-no-multicore-r1/result.json` | `c2772762480458352d3974f47f06ee2a5fd2e7082d55479ef9d0ab72a8d579bb` |
| No-multicore executable | `0ed83d39c2996cb449e02a52dbd91af9eb4b7760fae0a5782c163509a0e3b4af` |
| `indexed-reader-production-check-r1/result.json` | `eb2ac8e53cf51ad2e82b4c53c75a0677c62fbe6f24e146d5c8418b6af0e03159` |

Each test window retains `compile.json`, `artifact.json`, `groups.json`,
`test-0.json`, stdout/stderr and source maps. The default command is:

```sh
cargo iroha-fast -- test --manifest-path /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/Cargo.toml --locked --offline --target-dir /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/target --config 'patch.crates-io.halo2curves-axiom.path="/Users/takemiyamakoto/dev/iroha/vendor/halo2curves-axiom"' --lib --no-run --message-format=json
```

No-multicore adds `--no-default-features --features batch,circuit-params`. Artifact
features are `batch,circuit-params,default,multicore` and `batch,circuit-params`,
respectively, with debug assertions and overflow checks enabled. The recorded
executable runs `plonk::structured_key:: --nocapture`. The non-test check uses
`check ... --lib` with the same manifest, lockfile, target and patch; Cargo reuses
a fresh library artifact. These checks use the standalone vendor lockfile, not the
root Core dependency graph. Production, device and independent-review gates remain
open in [current readiness](../../../../specs/kagemusha_v1_production_readiness.md).
