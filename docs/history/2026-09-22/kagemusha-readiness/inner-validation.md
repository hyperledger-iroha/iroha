# Guarded inner IPA: scoped validation — 2026-09-22

The stored prover implements guarded outer and inner IPA and returns a closed
internal completed-proof owner. **The final component selection passes in both
build modes; KAGEMUSHA remains unqualified for production.** Authenticated
Core/native/SDK integration, final monetary artifacts, device limits and independent
review remain open. Work is in `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. Captured working-tree and build-input bytes identify each candidate.

## Final corrected candidate

Receipts under `target/kagemusha-validation/20260922/` record:

| Window | Test result | Compile / execution time |
| --- | --- | ---: |
| `inner-default-r4` | 27 passed, zero failed or ignored | 20.519 s / 56.547 s |
| `inner-no-multicore-r1` | Same 27 passed, zero failed or ignored | 25.991 s / 57.789 s |
| `inner-production-check-r2` | Non-test library check passed | 1.124 s |

These are **27 distinct functions and 54 test executions**. The nine groups contain
seven guarded-inner tests, two verifier controls, two ordinary prover controls,
five ordinary IPA commitment tests, one earlier outer-stage whole-PLONK fixture,
one multiopening verifier control, six bounded multiopening regressions, two FFT
identity tests and one IPA basis expansion test. Every captured before/after map
matches byte-for-byte across all three windows: 260 source/build inputs. Each test
binary remained unchanged during its window. The standalone vendor manifest and
lockfile are used; this is not root-workspace or authenticated Core integration.

The final candidate includes zero-u verifier rejection, empty/singleton identity
handling at both recursive-FFT entry points, and `compute_s([]) = [init]` for k0.
Both Pasta fields pass direct expansion and empty/singleton field/group regressions.
The test profile retains debug assertions and overflow checks. Reported durations
are local debug execution, not device proof-latency qualification.

## Retained unsuccessful windows

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

At this point, the two-entry-point FFT identity fix and two field/group tests
remained staged.
A pre-install source guard rejected a formatter-only reconstruction difference
before writing. A second worker nevertheless started against the unchanged
candidate: `inner-default-r2/setup-error.json` records this setup failure. Its
duplicate executions are not new corrected-candidate qualification. The corrected
selection narrows the ordinary filter to `poly::ipa::commitment::test::` and adds
the FFT tests, totaling 26 functions. Neither of these first two windows qualifies
the then-staged FFT change.

The second worker subsequently stopped after 24 executed functions: 23 passed,
the same k0 control failed, and the two unapplied FFT tests were absent. Its
captured inputs and executable remained unchanged. This repeats the seven inner
suites and adds thirteen passing ordinary-commitment, whole-PLONK and multiopening
regressions to the retained first window; it still does not qualify the correction
or provide an overall pass. `inner-default-r2/partial-result.json` has SHA-256
`b3e38c35ed75701e66b4d26ae6675181d0d87cdd6bf3aafd9b7a8be353f18318`.

After the FFT fix was installed, `inner-default-r3` compiled in 22.325 s and
executed all 26 selected functions: **25 passed, one failed, zero ignored**. The
same k0 acceptance control passed parameter construction, then panicked at
`src/poly/ipa/strategy.rs:183` on `assert!(!u.is_empty())`. The two new FFT tests
passed. Captured inputs and the executable remained unchanged. Its `result.json`
has SHA-256 `d3f14b3f3829c7ede93e946b5422fc37504ee744332b31f2d302a2fbb769e755`.
The only captured source/build-input change from this third candidate to the final
candidate is `poly/ipa/strategy.rs`, removing the empty-product assertion and adding
its direct expansion regression. The prior root lockfile change predates the third
window; no root dependency-graph execution is claimed from these vendor tests.

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
That earlier check establishes compilation of its own candidate only. The final
candidate's separate non-test check and both completed test windows are recorded
below; later changes require their own source maps and results.

| Final artifact (under the common receipt root) | SHA-256 |
| --- | --- |
| All final `sources-before.json` / `sources-after.json` maps | `2893fa2fa384bd3acd11fe049713f9df3ce1a14cd93a39cc3579fc4f74c17699` |
| `inner-default-r4/result.json` | `f245a8042c1c3da1773f4ab68d9aafdd35e10dfb6670052646c093b0f619b7d7` |
| Default executable | `1ddf251485647db086fcf9c0eddcc261fc169beceb567f40447932d751ea5653` |
| `inner-no-multicore-r1/result.json` | `2c6fbd301df1c7fb779b345537f05ee8408e3fd65f0a53b02824b041b8e8401f` |
| No-multicore executable | `a493762531af85951961a2ae383ae97be08359130b089d25cbe995dedb57cd2b` |
| `inner-production-check-r2/result.json` | `758f5185565bfcf7c87b6a74ccd75ea876878fabfef2c59d1c47a40456314220` |

The final default compile uses the command above. No-multicore adds
`--no-default-features --features batch,circuit-params`; its artifact confirms those
two features only. Each final window retains `compile.json`, `artifact.json`,
`groups.json`, `test-0.json` through `test-8.json`, stdout/stderr and full source
maps. The final production check uses `check ... --lib` with the same manifest,
lockfile, target and patch. It establishes non-test library compilation only.
