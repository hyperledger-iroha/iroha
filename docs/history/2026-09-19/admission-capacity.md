# Authenticated admission capacity — 2026-09-19

Work remained in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`, at
HEAD `87e0e45d17cc726e1afc9819746a652b8f694f20`. No commit, branch change,
compatibility path or sibling documentation change was made.

## Implemented boundary

The signed RS16 layout is the global proposal payload authority. Authenticated
active and terminal recovery validate the actual local resource projection before
publishing capacity or readiness. Local block, ready-body and per-source capacities
must cover that signed body; candidate selection refuses an undersized local
configuration instead of silently taking a smaller limit. Pending-Kura recovery
checks the same requirement before constructing its runtime consumers. The actual
configuration validator continues to establish disjoint source partitions.

The actual Sumeragi handle now checks the exact network/binding, active admission,
worst-quorum complete input and native descriptor, signed RS16 stripes, Control
publication frame, Consensus republication frame, and encrypted high-priority
queue charge. No request-supplied capacity or missing-handle fallback authorizes
receipt issuance. Production origin, forwarding and direct-receiver boundaries
apply this check. Origin sizing retains Torii's request-memory reservation;
post-quorum finalization rechecks the process owner and preserves an indeterminate
outcome if it is unavailable. Authenticated ingestion of an existing quorum
publication retains its existing State-owned custody and creates no new receipt.

These are necessary checks, not a complete future GLOBAL carrier guarantee.
Mandatory pulses, penalties and State-derived policies still require a bounded
coexistence contract, and retained autonomous anchors/optional evidence need an
owner-preserving selection rule. Later policy changes must not invalidate existing
durable promises. The existing headroom parameter is not that global guarantee.

## Validation

Combined build46 passes for Core, Torii, test-network, Kagami and daemon library/
binary targets plus `iroha_core_group_02` and `taira_consensus_contracts`, using
`scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test`
with `--no-run --message-format=json --locked --offline`. Runtime selection passes
115/115: 62 Core and 53 Torii exact tests. All 7,058 Rust/Cargo inputs and emitted
Core/Torii binary hashes remain unchanged. Coverage includes all candidate tests,
the nine capacity/startup/input checks, complete-input authentication and signature
sizing, original gossip and crash-temporary repair controls, actual journal receipt
and retry paths, pre-receipt refusal without journal mutation, and post-quorum
capacity-loss ambiguity. The prior small-carrier counterexample is now a refusal
at authenticated capacity validation followed by actual 800 KiB assembly under the
unchanged signed layout with a supported local resource configuration. It is unit
and handler evidence, not a signed-genesis/daemon campaign.

Build44 failed on two private topology-type imports in the new Core test fixture;
its emitted Torii artifact nevertheless passed all 48 selected tests. Build45 fixed
those imports, and its emitted Core artifact passed 62 tests, but the new Torii
post-quorum test called a Core-only test helper. Build46 uses the public exact
pending-input lookup and passes the combined build and entire selection. Both
failed build epochs retained all 7,058 captured inputs unchanged. Their runtime
observers correctly reported incomplete artifact selections, not combined success.

Formal46 passes 77/77 focused controls (41 admission-capacity, 22 inventory,
12 parser, two original-review controls) and the full canonical source-binding
gate with zero diagnostics. All 8,136 captured inputs, HEAD and index remain
unchanged. The companion binds startup publication, signed candidate limits,
pending recovery, active handle/network checks, actual envelope owners, both
production aggregator callers, request-memory ordering, and post-quorum refusal.
The preliminary formal44 run passed 33/35: two mutation anchors used stale
trailing-comma spelling; corrected fixtures retain the production predicates.
No model invariant or protocol safety rule was weakened. This does not rerun
all prior formal or runtime selections, full workspace tests, or real four/seven-
validator campaigns. All L1–L6 goals remain active.

Scoped Rustfmt, retired-codec guard and `git diff --check` pass. Raw receipts are
under ignored `dist/sumeragi-main-work/`: `validation46.json`,
`build46-source-{before,after}.json`, `admission-capacity-controls46/summary.json`,
and `formal-capacity46/summary.json`. The full gate ran with
`dist/sumeragi-main-work/venv/bin/python scripts/formal/check_sumeragi_v2_multilane_models.py --root /Users/takemiyamakoto/dev/iroha`.

## Superseded root prose

The latest combined Core/Torii/test-network/Kagami/daemon build43 passes, as do 542 distinct runtime controls: all 359 Queue tests, 150 geometry tests, eight publication-lock tests, the new small-carrier reproduction, 17 original review regressions, two pending-cache HTTP controls, and five delegated Kura/State owner controls. The capacity test confirms that an accepted 512 KiB configuration cannot carry a genuine approximately 800 KiB input which passes the protocol input bound; original durable custody survives both refusals. This is configuration/assembler unit evidence, not Torii ingress or daemon qualification. The canonical full source-binding gate and 21 final targeted checks pass on 8,133 unchanged inputs. The preceding 83 focused checks also passed before the final delegated-owner binding corrections; the older full 294-check selection was not rerun. Scoped formatting and the codec guard pass; workspace formatting retains unrelated differences. Full workspace tests and unchanged four/seven-validator qualification remain outstanding. Production native runner cutover, original Validate-to-Apply custody, pre-vote capacity policy, pending geometry/Queue retirement, participant durability and carrier-proof custody remain separate open outcomes. Native DA/pin/SCCP payload execution remains unsupported; complete owners, broader NPoS evidence/penalty and sponsor/lease coverage, aggregate resource admission and native Windows namespace durability remain outstanding. No L1–L6 goal is complete.

The next Sumeragi cutover must carry the implemented consuming State publisher and exact retained output owner through the complete production Validate-to-Apply path, including cached/recovered markers and voting; preserve one original execution per exact subject. Keep original World/runtime journals, source groups, witness, result bytes, exact lease-bound checkpoint receipt and verified QC/Kura authority joined before visibility. Carry the pristine QueuePlan/NPoS control owner, authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks into the canonical validator while preserving State generation and complete carrier custody. Retain the implemented Queue cut and original Kura guards through pending geometry/Queue retirement and participant durability; bind them to the exact State/Queue pair from V2ApplyService and keep later State/component acquisition try-only. Bind complete mandatory framing and signed RS16 feasibility to admission before durable receipts; the accepted-small-carrier reproduction now demonstrates why the protocol input cap alone is insufficient. Finish bounded pre-vote descriptor/capacity admission, capture, file-handle and installation resources, and reachable typed deferral. Capacity scanning must precede geometry/sidecar acquisition; public geometry/GC/history wrappers cannot run under the held joint lease. Preserve original route-directory custody during retirement and account for witness staging on physical retries. Keep obsolete State observations as typed refreshes, including a finalized height advance, and preserve independent beacon and key-lifecycle behavior. Complete and qualify Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease coverage before opening the live Native header gate. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded history references, retaining exact source/finality/WSV joins and whole-bundle accounting. Defer physical GC behind release, snapshot/recovery pins and an instance-local deletion fence; qualify native Windows namespace durability separately. Connect the process-lived shared lane reducer to transport and canonical Decision application while retiring the old fresh-signing authority in one complete cutover. Preserve cross-route closure, immutable incarnation storage, drain/signing fences and historical native authority. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate and the full workspace before closing any liveness goal. Source-scoped evidence lives in [status](status.md); [superseded local plans](docs/history/2026-09-19/merge-source-publication-evidence.md) are preserved verbatim.
