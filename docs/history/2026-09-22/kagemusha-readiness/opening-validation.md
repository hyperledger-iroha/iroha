# Stored IPA opening validation — 2026-09-22

Guarded outer multiopening through P is implemented and has scoped default-feature
validation. **KAGEMUSHA remains unqualified for production.** Guarded inner IPA,
authenticated Core/native/SDK integration, final monetary proof artifacts,
physical-device resource limits and independent review remain open.

## Completed default windows

Both windows ran in `/Users/takemiyamakoto/dev/iroha` on `optimizations`, with
context HEAD `6153ef6558c7339ecaf4634447ac246ed1a9455f`. Captured working-tree and
build-input bytes identify the candidates; the commit alone does not. Receipts
are under `target/kagemusha-validation/20260922/`.

| Window | Selected functions | Result | Source/binary stability |
| --- | ---: | --- | --- |
| `opening-default-r1` | 42 | 41 passed, 1 failed, 0 ignored | Unchanged; 254 captured inputs |
| `opening-default-correction-r2` | 2 | 2 passed, 0 failed, 0 ignored | Unchanged; 255 captured inputs |

Together these record **43 distinct passing test functions across two windows**,
with 44 executions including the original failure. They are not one 43-test run.
The first selection covers scalar/opening (12), retirement (12), quotient
commitment (1), selectors (5), indexed-key scanner (5), ordinary bounded
multiopening (6), and singular-challenge verifier handling (1).

The initial failure was the test's event-count assertion at former
`opening_tests.rs:1149` (10 observed versus 9 expected) after receipt drift during
`Field::random`. One field sample invokes multiple infallible RNG subcalls before
returning to its enclosing owner check. The correction permits only those
remaining subcalls and requires rejection before the next protocol step, with
no quotient commitment, u scalar, later challenge or prepared P observed. Panic
and write-error prefix assertions remain strict. Production code did not change.
The corrected protocol test passed in 65.815 s; the new satisfiable test passed
in 14.057 s. The original failure remains in the first window's logs and result.

Comparing the completed source maps finds exactly two differences: modified
`proof_evaluations/opening_tests.rs` and added `proof_evaluations/satisfiable_tests.rs`.
All captured production/dependency/build inputs are identical. The exhaustive
source-read fault matrix and dense P parity both passed in the first window.

## Proof evidence and its limits

The original differential fixture uses `InverseCircuit`, whose unconditional
constant-one gate deliberately makes it unsatisfied to exercise inverse tails.
It passes dense grouping, initial evaluation, Q/Kate/q-prime/P and blind parity,
complete output-byte equality, transcript/RNG agreement and direct IPA opening
verification. These checks are **not positive whole-PLONK acceptance**.

The separate satisfiable square/lookup/copy fixture calls the real whole-PLONK
`verify_proof` with `SingleStrategy`. Dense and stored-stage outputs are identical
and verify in both Pasta fields with direct, default and hybrid instance policies:
six cases inside one Rust test function. Changed values in each public column,
corrupted proof bytes and truncation reject. The stored stages reach guarded P;
a **test-only adapter then uses the ordinary unguarded inner IPA**. This establishes
a valid small-circuit proof trajectory, not a production complete guarded prover,
a final KAGEMUSHA recursive monetary artifact, or a process-RSS/latency bound.

Additional passing controls cover first-encounter source/point/set identity,
duplicate queries before evaluation, zero PLONK-challenge collisions, every Kate
step and erased physical tails, checked payload overflow, and exact-budget
admission. The failure matrix covers every observed source read with an error,
early/middle/late malformed/read-unwind/drift cases, each observed protocol unwind,
point/scalar write errors, sampled protocol drift, owner destruction and sentinel
survival. The verifier repair returns `OpeningError` for singular x3-minus-point;
its forced-challenge tests verify rejection without resampling and a normal real
IPA proof's acceptance. It is robustness hardening, not a demonstrated practical
challenge-grinding vulnerability.

## Receipts and reproduction

| Artifact | Initial default SHA-256 | Correction default SHA-256 |
| --- | --- | --- |
| `result.json` | `74b5944ab3c3ccc791838d848c341a52d7c3538e78041f7be600f83e127bc7ba` | `f7d6f66034888fad82c5b82f056daf83489a5ed694e0ae9198f8099a42cd982a` |
| Before/after source maps | `bd250e16e96b217ff575d9cce6d8cc5babfbd283756063bc1467d9e28bc0e835` | `12e46cd9a330cd986f5c4ae1d31057d3dba0240647ffac2b0d79628903aeec06` |
| Executable | `03740f670041f0556f896b1e2f052aae88cd51e054b871565e61abb737f6cf14` | `d14b47233fdfd9dcca2bff8cd13bc768301e2cdccbf5b430200a89155ac17fe6` |
| `compile.json` | `cec5611a8cd09facaad2971d1d31ba0c88d751d84e9835a52ec36b4b01abc4cd` | `ebdd626b86191de4ea7dc4ce68f7fd5bb3b33f63d7975f20b05fb38a625c4e33` |
| `artifact.json` | `5ec7fb1367fd45703f8679ec3f5a58f4c12856b2e34533e79b89fab1c280b1dd` | `fff08319cf254ab0022b048c19e4f8a999b3105c27af454d03ab49909b310f69` |

These are standalone vendor-library tests using the vendor manifest and lockfile;
they do not compile or qualify the root workspace or Core integration. Each
`compile.json` records this exact successful command; builds took 32.645 s and
148.237 s respectively. Features were batch, circuit-params, default and
multicore. Both artifacts used the same executable path, so the newer compilation
replaced the initial local binary; the separate receipts retain each run's hash.

```sh
cargo iroha-fast -- test --manifest-path /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/Cargo.toml --locked --offline --target-dir /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/target --config 'patch.crates-io.halo2curves-axiom.path="/Users/takemiyamakoto/dev/iroha/vendor/halo2curves-axiom"' --lib --no-run --message-format=json
```

`groups.json` and `test-*.json` retain exact executable/filter/`--nocapture` argv;
`test-*.stdout` and `test-*.stderr` retain summaries and injected/actual failures.
The two correction filters are
`both_pasta_stored_opening_all_protocol_unwinds_and_write_errors_stop_and_clear`
and
`both_pasta_satisfiable_stored_square_lookup_copy_proofs_match_dense_and_verify_all_instance_modes`.

## Completed no-multicore rerun and interrupted attempt

`opening-no-multicore-r1` stopped without `result.json`, `groups.json`,
`test-0.json` or a final test summary. Its nine partial success lines do not
qualify the selection. The process/session disappeared; the cause and terminal
exit status are unknown. This attempt remains **interrupted/unqualified**.
Its retained group-0 stdout SHA-256 is
`022093d7e3d39066ef7e362d7bc44a650ecebc4e7f0c441d47c834c0e8006832`
and stderr SHA-256 is
`3ce6cff9d1f9f15181b1cdd21ffcc482ad4a7371478dcfdf2abe3dc7f478f6a8`.

The durable `opening-no-multicore-r2` rerun completed **42 passed, zero failed,
zero ignored**, with unchanged source and executable bytes. All seven raw group
summaries match the selection, and the 255-entry before/after source map exactly
matches the completed default correction map. Its selection includes the corrected
protocol matrix and six-case positive whole-PLONK fixture. It explicitly excludes
`both_pasta_stored_opening_all_observed_reads_and_budget_failures_destroy_original_owners`,
which already passed in the initial default window; it is not a no-multicore
pass for that exhaustive matrix. The union remains 43 distinct passing functions,
with 42 of them also executed successfully without multicore.

| No-multicore rerun artifact | SHA-256 |
| --- | --- |
| `result.json` | `09f96cb0ee7e537cc302f81d0457bc3fbc2698e64a8fc305b0622e8e0e4c7132` |
| Before/after source maps | `12e46cd9a330cd986f5c4ae1d31057d3dba0240647ffac2b0d79628903aeec06` |
| Executable | `87a8010adf413a067e3ad6bcc875d0fa4461085e9500e8fc87f2c46f67f23d1e` |
| `compile.json` | `2fcdf24f87acbf047cb27800b73f05ffd21e7d6895777043191b324ff0a0c5d3` |
| `artifact.json` | `0e3b6db9f31212fc76de8aecaee0a373056d4948117769a7d41890fbac491119` |

The exact compile command above adds
`--no-default-features --features batch,circuit-params`. Cargo returned the cached
executable in 0.659 s; this was not a fresh source rebuild. The artifact is
`vendor/halo2-axiom/target/debug/deps/halo2_axiom-cf047cd5e9d47e25`.
Group execution times were 110.306, 20.526, 117.684, 2.499, 5.555, 3.550 and
0.910 s in the selection order. These remain standalone vendor-library tests.

All completed receipts here bind the guarded-P candidate. Later guarded-inner-IPA
implementation or verifier changes are outside these windows. Neither mode
qualifies authenticated Core integration, monetary artifacts or hardware.
The previous pending wording is retained byte-for-byte in the
[no-multicore predecessor record](opening-no-multicore-predecessor-note.md);
the [manifest](opening-no-multicore-predecessor-manifest.json) also authenticates
the exact replaced readiness paragraph and KAGEMUSHA status row.

The [earlier scalar/blind receipt](scalar-blind-validation.md) remains separate.
Exact superseded current prose is preserved in
[active implementation](opening-predecessor-active.md),
[validation](opening-predecessor-validation.md) and
[status row](opening-predecessor-status-row.md), with byte hashes in
[the predecessor manifest](opening-predecessor-manifest.json). These archives
record the update, not extra tests. See
[current readiness](../../../../specs/kagemusha_v1_production_readiness.md).
