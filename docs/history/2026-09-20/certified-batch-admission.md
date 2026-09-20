# Shared certified transaction admission

Work is confined to `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`,
HEAD `482a02fa708fb83f00b3cadd4067ad8f0b641b49`. No compatibility path, worktree,
commit, staging operation or sibling documentation edit is part of this change.

## Implementation

The public transaction batch previously pushed Ordinary economic inputs directly
into Queue while rejecting QueuePlan input. It now uses the same prepared-route
and durable-dispatch owner as public single and entrypoint submission. Complete
bounded decoding, authentication, current fresh policy and route preflight finish
before the first dispatch. The only Ordinary exception is the same authenticated
threshold-key lifecycle certificate used by single submission. The independent
local batch writer and its synced-intent rejection are removed.

Batching aggregates transport; it grants no atomic distributed admission. HTTP
202 acknowledges every input. Once dispatch begins, a failure produces HTTP 207
with every original signed hash, individual status and rejection/ambiguity code
in input order. A later refusal cannot conceal earlier durable acceptance.
Physical writes already in progress cannot be preempted by the batch deadline;
the deadline stops new dispatch and bounds cancellable transport waits. A lost
dispatched response remains explicitly indeterminate.

Canonical custody is checked after actual signature/network authentication and
before fresh TTL, crypto policy, route and capacity checks. Preflight is a
snapshot: dispatch refreshes canonical and current durable custody before charging
authority quota. A globally bound original claim preserves its route and bypasses
fresh queue capacity. Duplicate entries at capacity one and burst one retain the
same journal claim, including a subsequent single-submit retry. Fresh entries
reserve quota individually and commit it only after an accepted response.

`TransactionBatchEntryOutcome` owns the Norito/JSON response shape. Rust returns
a typed partial/ambiguous error retaining original hashes and any verified ordered
outcomes. JavaScript verifies ordered identities against the native canonical
transaction hasher and exposes partial outcomes. Both refuse contradictory counts
or substituted identities and do not automatically resend a batch. The three
OpenAPI copies and the source-coupled corridor reference describe this contract.

## Fixture corrections preserve production authority

Fourteen older single-submit fixtures now use real four-authority QueuePlan
admission with local and authenticated HTTP-peer journals, or the genuine
threshold-key lifecycle exception. No Ordinary economic bypass was restored.
The shared SCCP fixture used to install results before changing its commitment
root; that setter invalidates results. It now fixes the root first, installs the
complete original output, and then derives the final header signature and QC.
Result, cached Merkle-root and recomputed SCCP-root consistency are asserted.

An unfunded fee test now verifies the actual authority's precise HTTP 422/code
before testing the public proxy. The proxy's HTTP 503 outcome-unknown response is
required by its existing authenticated-evidence contract: an HTTP rejection alone
cannot prove absence of custody at every dispatched authority. Both queues and
committed/Explorer history remain empty in the fixture. No unauthenticated
negative certificate or duplicate production fee preflight was introduced.

Two routing spies now observe the StateView method used by production. That
exposes both initial route capture and Queue's required current-plan equality
check; a raw supplied plan is not proof that current policy still authorizes it.
A deterministic route change between those reads fails before durable admission.
The recent-SCCP test now requires the original authenticated finality archive: missing/corrupt evidence
fails, while genuine publication preserves message identity, links and stable
projection bytes. The obsolete metadata-only acceptance assertion was removed.
The old batch quota-commit wrappers were also deleted; their existing atomic
rollback assertions now call the surviving production reservation owner.

## Validation history

Builds75 and76 compiled seven packages and selected integration targets with
unchanged Rust/source assets, HEAD and index. Build75 took 493.132 seconds; its
initial selection passed 37/41. The four failures exposed the resultless SCCP
fixture and the obsolete public fee-response expectation. Build76 took 192.089
seconds; its expanded selection passed 115/118 (112 Torii, two Rust client, one
data-model). The remaining failures were two route spies observing a retired
router method and an obsolete recent-SCCP test allowing missing finality. Those
failures are retained in the local evidence rather than reported as passes.
Build77 compiled in 84.358 seconds with unchanged Rust inputs. Its selection
again passed 115/118: the repaired route spies exposed the required second
classification, and the SCCP corruption test still named a retired sidecar path.
Neither failure justifies removing current route or retained-finality validation.
Build78 caught five remaining test callers of the changed router-fixture helper
signature and failed compilation in 12.008 seconds. All five callers in part3
were updated together; no runtime result is claimed for that build.
Build79 then compiled in 41.219 seconds with unchanged Rust inputs; 118/119
controls passed. Both actual route revalidation positives and the new
pre-persistence drift rejection passed. The sole failure was the recent-SCCP
fixture's missing retained governed route, after exact finality authentication.
The reader's destination-binding/configuration/lane joins remain mandatory.
The corrected endpoint fixture reuses the governed archive construction before
its separate eviction phase. Transaction, State and original signed finality
share the exact Taira network identity; the deliberately ungoverned fixture
continues to test missing-binding rejection. An independent build79 probe of
`bundle_proof_request_and_recent_readback_survive_body_eviction` failed while
advertising replicas before eviction; that separate failure is retained and
was diagnosed as missing authenticated physical geometry at identity binding.
Its missing-finality negative control passed. Build80 compiled in 77.560 seconds
and passed all 119 admission/SDK/SCCP endpoint controls on unchanged binaries;
the separate old eviction probe still failed. The shared archive fixture now
creates configured Kura and exact-network State at height zero, authenticates
the configured primary geometry and restores lane segments before writing its
three blocks. All original eviction and readback assertions remain. Build81
compiled the same seven-package scope in 75.152 seconds with unchanged Rust
inputs. All 31 original-review regressions pass; its expanded 142-test selection
passes 141. The archived-body readback and endpoint now both pass. The remaining
failure was an obsolete decoder assertion treating the all-zero replay root as
a sentinel, contrary to the current full-width hash contract. The dedicated
replacement preserves exact zero/nonzero decoding, proves both stale roots fail
authoritative forest verification and occupancy without mutation, proves the
valid witness succeeds, and retains occupied/noncanonical decoder negatives.
No production decoder or replay validation rule changes.
Build82 compiled in 76.067 seconds; all 32 Core review/replay controls passed.
The expanded ingress selection passed 142/143: one token-identity test's real
admission took long enough for its one-token-per-second bucket to refill. Eight
single/batch identity, capacity and quota fixtures now use a test-only finite
burst with zero refill through the original limiter. The regression advances
bucket time by a day and still rejects the spent principal while accepting a
different principal. Production refill and rate configuration are unchanged.
Build83 compiled that constructor and the two token-identity callers in 47.930
seconds; its runtime was not used as final evidence. The other six admission
callers are included in build84.

The build82 full canonical structural gate reported 30 diagnostics while its
230 focused controls passed on unchanged inputs. Fifteen new batch-owner tokens
had been entered as normalized strings, but two full-gate consumers require
literal source substrings. The checker and ledger now carry matching exact
source text, retaining the same normalized obligations. A new literal-owner
positive supplements the semantic mutants and catches this disagreement. The
failed gate and all outputs remain at `batch82-formal/final`; no pass is claimed
for it.

Local receipts are under `dist/sumeragi-main-work`: `build{75..84}-receipt.json`,
`build{75..84}-source-{before,after}.json`, `core-build-{75..84}.jsonl`,
`batch75-new/summary.json`, `batch76-full/summary.json` and
`batch77-full/summary.json`; build77 uses the same receipt naming. Source-manifest SHA-256:

| Build | Before/after source manifest |
| --- | --- |
| 75 | `59f67ba384c7a51b52723ec29521f9d428f93458d291756206891dc891c4f552` |
| 76 | `ec7e2a0d3ca3ea67c4a8f168d3fe8e55832eb68221fec90f01bdfb009ddffa94` |
| 77 | `0190d5c7f0859545c2489c80a51dc6ffe0b5497ee32f6f57b045caa1934ce0fc` |
| 79 | `e32929774a401494ee8c5f0a0b1793533f64e33cc157581574215cd433964b9a` |
| 80 | `8d4c7101fe705315d8e60b8241a0900965c33e47b9129a060b5996b07a52267c` |
| 81 | `9b8c0ae22086a2e112bd805040b3f8dca80f981e3dcf5c9679a34792f91996ad` |
| 82 | `d77a6d2a311364603daa9778db720a9d01a760b7647ceffd50ad1f6200fb5a08` |
| 83 | `d5d7b7091c189874ddf11179ed57ea12f1cb35516d3ce2b2ce3f58ec134fcd95` |
| 84 | `a516420d3e4c12eeab0ae33bce70f3ddb11ac4328206e3ae7b4dd7ab02a9a934` |

The compile command, with `DEVELOPER_DIR=/Library/Developer/CommandLineTools`:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test \
  -p iroha_core -p iroha_torii -p iroha_data_model -p iroha_test_network \
  -p iroha_kagami -p irohad -p iroha --lib --bins \
  --test iroha_core_group_02 --test taira_consensus_contracts \
  --no-run --message-format=json --locked --offline
```

The dedicated JavaScript response-parser selection passes two tests, including
ordered hash/status/count substitutions and no automatic resend. It injects the
codec/HTTP owner and a prior capability observation; it does not qualify the
native addon. The broad existing Torii JavaScript suite could not start because
the native addon is unavailable. The codec dependency guard, exact agreement of
the three OpenAPI batch operations and historical archive verification pass.

Build81 passes all 31 focused original-review regressions, covering sealed
complete-input identities, real two-MiB carrier fitting and deferred custody,
one-copy certified gossip, actual synced repair-temporary recovery and
foreign/tampered/unowned temporary rejection. The Core executable is unchanged
through that selection. `review81/summary.json` records every exact test and
its output; this supplements the earlier broader build71 recovery evidence.

The final build84 reruns the same seven-package compile command. Its expanded
ingress selection includes all 24 SCCP first-release API controls, plus the explicit no-refill control, for 144 exact
tests across Torii, the Rust SDK and data model. Its authoritative result is
`batch84-full/summary.json`, with binary hashes before and after execution.
`review84/summary.json` reruns the original 31 controls plus the actual Core
`submit_native_transfer_proof_requires_sparse_replay_witness` settlement test,
which rejects stale roots without changing balances, proof state or replay state.
The final structural source check and 231 canonical-retry/pending-membership-ledger
controls use `batch84-formal/final/summary.json`. That capture includes the changed SDK,
OpenAPI and source-adjacent documentation as well as Rust, formal and script
inputs, and checks unchanged branch, HEAD and index. These are structural and
source-binding controls, not a protocol-proof qualification. This document is
itself an input to that final frozen capture; the result belongs to its receipt.

The current JavaScript parser selection passes 2/2 and scoped ESLint passes.
The codec guard and historical archive verification pass again; the latter
authenticates 64,736 records and 67,311 occurrences.

## Remaining scope

This is scoped admission evidence. Other generic economic adapters still require
migration. Native production ingress remains closed pending the process-lived
runner, complete resource admission and original Validate-to-Apply owner cutover.
The full workspace, complete SDK/native-addon suite and unchanged four/seven-peer
fault/restart/final-transaction qualification remain open. No liveness goal or
release gate is closed by these focused results.
