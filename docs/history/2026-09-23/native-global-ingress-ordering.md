# Native/global ingress scheduling correction

This record covers local development validation in `/Users/takemiyamakoto/dev/iroha`
on `optimizations`. L1–L6 remain open. It does not qualify a clean signed release,
the complete fault/seed campaign, or the two historical NPoS fail-stops whose
initiating errors were not captured.

## Root cause and current rule

The global leader-wire gate treated every physical predecessor as a global
dependency, including Native custody with process lifetime. Native polling can
retain a backpressured occurrence while another remains queued. The finalized
drain deliberately leaves that Native queue intact. Consequently, it could return
`None` while a later global carrier remained queued; both terminal callers then
failed the strict global-empty cut with
`finalized global ingress cut retained global ownership`.

`fair_v2_ingress_queue_gate_verdict` now distinguishes eligibility from the full
physical ownership census. The exact global carrier waits for its global
predecessors. Native remains independently eligible and retains its original
allocation, bytes, physical ordinal and authentication. Corrupt Native predecessor
geometry still fails before the downstream predicate runs. Later global carriers
cannot overtake the first global carrier.

Independent Native traffic uses the existing rotated dependency pass while a
global carrier is active. Giving a replenished Native source strict priority
would otherwise starve a matching Proposal/body completion behind a blocked
global Vote. The regression refills Native after each dequeue and requires the
global dependency within one two-source rotation, through both checked dequeue
and the production lifecycle-cut selector. Global strict work retains priority.
No queue-clear escape, second selector, compatibility path or relaxed global-cut
authentication was added.

## Counterexamples and validation

- Core100 retains the original production selector. All four domain regressions
  fail at their intended assertions, including the actual signed four-validator
  Native fixture with driver capacity one, a retained second control, a queued
  third control and a later signed global TimeoutVote. Core101 passes all four.
  Core99's first attempt also exposed a zero timeout-reserve fixture error; that
  result is retained and is not used as the authenticated terminal counterexample.
- Core105 reproduces starvation with independently eligible Native traffic at
  strict priority. Core106 passes the corrected bounded-service test as well as
  the four original domain regressions.
- Core106's expanded selection passes 877/878 controls. The sole failure expects
  all Native ingress to remain unsupported until a future runner cutover. The
  retained Core96 binary fails that test identically, with identical admission
  implementation and test bytes. Its replacement checks closed-queue rejection
  without ordinal consumption, followed by exact Native admission/dequeue after
  opening, with no global lifecycle authority. Production code is unchanged by
  this test correction.
- Final Core109 passes all 878 controls on ordinary stacks, including the six
  new/strengthened scheduling and admission tests and the original review
  regressions. Its final build/source/executable join is true; listed inputs,
  binary and Git metadata stay unchanged. Core109, daemon110 and harness111 all
  compile successfully from the same captured source.
- Final canonical multilane structural gate (`multilane-current8`) and all 100
  Native contract/mutation controls pass with unchanged captured inputs. These
  are structural checks, not a runtime liveness proof.
  The three changed exact gate-digest consumers separately accept the reviewed
  predicate and reject twelve mutations, including removal of dependency fairness.
- Changed Rust files pass formatting. The retired-codec guard passes. Workspace
  formatting reports existing SoraFS ordering/wrapping differences in
  `crates/iroha_core/src/smartcontracts/isi/mod.rs` and
  `crates/iroha_data_model/src/isi/mod.rs`; that check is not reported as passing.

The daemon107/harness108 pair, before the admission-test-only correction, passes
the real four-validator NPoS leader-outage/restart test in 226.640274 seconds and
the real seven-validator two-restart test in 320.761060 seconds. Both require a
real network, use one startup attempt, have no ignored cases, and retain unchanged
listed source inputs, binaries and Git metadata. No diagnostic exit-delay library
was injected. The seven-validator case exercises Native execution before and
during the two-validator outage, recovery from both retained stores and a final
finite input on all seven validators. These passes do not establish the cause of
the older NPoS failures.

The final daemon110/harness111 pair passes the four-validator silent-author,
sole finite input and genesis-restart diagnostic in 113.155621 seconds, with
unchanged listed inputs, binaries and Git metadata. Both final binaries are
byte-identical to daemon107/harness108; the sole intervening captured Rust change
is the admission test in `mod_authoritative_runtime_gate_07_wire_bounds.rs`.
`generation249-native-admission-guard/unchanged-network-binaries.json` records
that comparison without relabelling the earlier whole-source captures.

The three network log SHA-256 values are, respectively:
`1bcdb4dffce6ce14e738e481f8bc6338bcc83668d47b6e511b623f50ce575776`,
`af8bcc60969ea9fc75b777d52bd975eb0ae0fbe6cfa402f55ad6f975c5dbb959`,
and `916d384df11f21451123dc676b91499783d31ab0df43745e3ddffcdec1a58b69`.

## Retained evidence

Build receipts and Cargo artifacts are under
`dist/sumeragi-main-work/generation167-checkpoint-binding/`:
`checkpoint-native-domain-red100`, `checkpoint-native-domain-core101`,
`checkpoint-native-priority-red105`, `checkpoint-native-fair-core106`,
`checkpoint-native-fair-daemon107`, `checkpoint-native-fair-harness108`, and the
final `checkpoint-native-final-{core109,daemon110,harness111}` captures.

Runtime selections, individual logs, binary hashes and source joins are under
`generation169-merged-regressions/`; network captures are under
`generation188-real-process-build/{npos-timeout107-local1,two-restarts107-local1,silent-author110-local1}`.
`generation244-native-global-ordering`, `generation246-native-domain-contract`,
`generation247-native-domain-validation`, `generation248-native-dependency-fairness`
and `generation249-native-admission-guard` retain the reviewed diffs, exact
preimages, counterexamples and formal controls. Formal current-source receipts
are under `generation234-current-formal-gates/`.

Each prefix above is below `dist/sumeragi-main-work/`. Full runtime joins include
listed Rust/manifests/fixtures and retained executable identity. They do not seal
every compiler/environment input or satisfy the release artifact contract.
