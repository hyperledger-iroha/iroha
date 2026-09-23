# Guarded inner IPA: partial validation — 2026-09-22

The stored prover now implements guarded outer and inner IPA and returns a closed
internal completed-proof owner. **The first regression window is incomplete;
KAGEMUSHA remains unqualified for production.** Authenticated Core/native/SDK
integration, final monetary artifacts, device limits and independent review remain
open. Work is in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`; context
HEAD is `6153ef6558c7339ecaf4634447ac246ed1a9455f`. Captured working-tree and
build-input bytes, not HEAD alone, identify each candidate.

## Actual first-window result

Receipts are under `target/kagemusha-validation/20260922/inner-default-r1`.
Vendor-library test compilation passed in 78.461 s. The partial receipt records
**11 functions executed: 10 passed, one failed, zero ignored**, with all 259
captured inputs and the executable unchanged:

| Executed group | Result | Duration |
| --- | --- | ---: |
| Guarded inner IPA: four private units and three real-owner integrations | 7 passed | 178.193 s |
| Inner verifier controls | 1 passed, 1 failed | 1.018 s |
| Ordinary IPA prover controls | 2 passed | 0.071 s |

The failed `both_pasta_inner_ipa_nonzero_rounds_and_zero_round_count_still_verify`
control reaches k0 parameter construction and panics at `src/fft/recursive.rs:371`
by indexing an empty stage list. Zero-u rejection passed. The runner then stopped
because `commitment::test::` also selected two KZG tests. The remaining groups did
not execute; this is not a 24-function pass or complete regression qualification.
The seven inner tests pass despite this separate FFT boundary defect; their
success does not erase the failed control.

The two-entry-point FFT identity fix and two field/group tests remain staged.
A pre-install source guard rejected a formatter-only reconstruction difference
before writing. A second worker nevertheless started against the unchanged
candidate: `inner-default-r2/setup-error.json` records this setup failure. Its
duplicate executions are not new corrected-candidate qualification. The corrected
selection narrows the ordinary filter to `poly::ipa::commitment::test::` and adds
the FFT tests, totaling 26 functions. Corrected default and no-multicore
qualification remain pending; no result here qualifies the staged FFT change.

The second worker subsequently stopped after 24 executed functions: 23 passed,
the same k0 control failed, and the two unapplied FFT tests were absent. Its
captured inputs and executable remained unchanged. This repeats the seven inner
suites and adds thirteen passing ordinary-commitment, whole-PLONK and multiopening
regressions to the retained first window; it still does not qualify the correction
or provide an overall pass. `inner-default-r2/partial-result.json` has SHA-256
`b3e38c35ed75701e66b4d26ae6675181d0d87cdd6bf3aafd9b7a8be353f18318`.

## What passed, and what it establishes

The satisfiable square/lookup/copy fixture uses `finish_guarded_ipa` and the real
whole-PLONK verifier. Both Pasta fields in direct/default/hybrid instance modes
produce the same proof bytes, transcript events and next RNG output as dense
proving and verify: six cases inside one passing Rust function. Changed public
inputs, corrupted proof bytes and truncation reject. This qualifies those small
generic circuit cases, not final KAGEMUSHA recursive monetary artifacts. The
previous [outer-stage positive fixture](opening-validation.md) instead used a
test-only ordinary inner IPA; its historical scope remains unchanged.

The differential fixture deliberately retains an unsatisfied inverse circuit.
Its k4/k8/k9 comparison with the original inner prover, xi/z/nonzero-u edges and
zero-u rejection at every k4 round establish arithmetic, transcript/RNG and
failure-prefix equivalence; they do not establish positive whole-PLONK acceptance.
Private units compare collapse with original and independent arithmetic at
k1/k4/k8, check erased physical tails and stable allocations, reject invalid
bounds/zero challenges and capacity overflow, and compare guarded joined MSMs
with the ordinary oracle, including empty-half k0 and Drop/unwind erasure.

The fault test rejects receipt drift at a late callback in every one of the 25
complete k4 field samples. It injects RNG panic at six representative sample
sites, panic/drift at every non-RNG protocol boundary, errors at every point/scalar
write, and zero/minimum-minus-one budget or initial drift before protocol effects.
A drifted field sample may finish its own eight infallible RNG callbacks, then
must reject before any adjacent sample or protocol step. Owner destruction,
erasure and unrelated sentinel survival pass. This is not exhaustive panic
injection at every RNG subcall or backend-internal erasure evidence.

The verifier rejects a zero round challenge before batch inversion and final
scalar reads, matching the original prover without resampling. This is edge-case
robustness, not evidence of a practical transcript-challenge exploit.

## Guarded payload boundary

The consuming continuation retains the original key, parameters, RNG, transcript,
receipts and derived blind through final c/f writes. Its closed crate-private
completed owner exposes no detached proof/key/RNG/transcript accessor. P is erased
after its last use and reused as b; S becomes P'. Both physical arrays remain
initialized at n elements while erased tails leave a shrinking logical prefix.
The reusable guarded MSM join preserves ordinary scalar/base order and its single
backend call.

Minimum initialized field-array storage is **2n + floor(n/2) + 2 slots**; owned
affine arrays contain **n + floor(n/2) + 2 slots**. Checked admission charges the
actual capacities of all five arrays plus `sizeof(Workspace) + sizeof(Column) +
sizeof(SecretLookupBlind)`, before the first random sample. Explicit headers
include the six guarded scalar slots and incoming derived blind; the array count
alone is not total live field memory. Inherited key/parameter/provider/transcript/
receipt allocations, backend scalar representations/caches, generator-collapse
projective and batch-normalization scratch, allocator overhead, general stack and
compiler/register copies are excluded. No complete-process erasure, peak RSS,
128 MiB ceiling or device latency result follows from this component accounting.

## Receipts and commands

| First-window artifact | SHA-256 |
| --- | --- |
| `partial-result.json` | `4b113546886e3459dba53cf72d92b251f6ff13e93a502f0294c7f70dc65d9bda` |
| `sources-before.json` / `sources-after-partial.json` | `b77ca7a4bde5f12b10ba17bfbe5c558a3fa8ed76acf6e0869931e91783608b35` |
| `compile.json` | `f8c515a188575b0fb93ec79ec2468e58dc0a186547e6e0a2b520c36c17653b8d` |
| `artifact.json` | `6e5e62d7df2501cd0fa0aea631d34e28d6aa72df7f8f8ac9664181a669a9fc47` |
| Executable | `53198d232965db6147aeff6569d57990b5fc774af640e0eb560a4bccb045bbf1` |
| Second-worker `setup-error.json` | `57f5588ce78bf3c51d6e96036202547481e467d708327f16e9e9288bca234a50` |

These are standalone vendor-library tests using the vendor manifest/lockfile,
not root-workspace or Core integration. The exact compile command is:

```sh
cargo iroha-fast -- test --manifest-path /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/Cargo.toml --locked --offline --target-dir /Users/takemiyamakoto/dev/iroha/vendor/halo2-axiom/target --config 'patch.crates-io.halo2curves-axiom.path="/Users/takemiyamakoto/dev/iroha/vendor/halo2curves-axiom"' --lib --no-run --message-format=json
```

`test-0.json` through `test-2.json` record executable/filter/`--nocapture` argv;
stdout/stderr preserve summaries and the actual panic. Features are batch,
circuit-params, default and multicore; debug and overflow checks remain enabled.
A separate non-test production vendor-library check passed in 9.112 s using the
same manifest, lockfile, target and patch with `check ... --lib`, and unchanged
captured sources. `inner-production-check-r1/result.json` has SHA-256
`a9065397780dc9a5272a5a85d750d5d3b46a5954425c89546b9eced0bed728c5`.
It establishes production compilation only. Later candidate changes require their
own immutable source maps, executable receipts and completed test results.
