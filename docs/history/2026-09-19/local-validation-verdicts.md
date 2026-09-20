# Local validation failure versus durable verdict — 2026-09-19

Work remained in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`, at
HEAD `87e0e45d17cc726e1afc9819746a652b8f694f20`, with unchanged index. No branch,
worktree, commit, compatibility path or sibling documentation edit was created.

## Corrected ownership boundary

The previous BodyValidationError default classified every non-sidecar failure
as deterministic rejection. The live body store could therefore persist an
invalid-body marker for a local Kura failure. Cold replay could also reproduce
a rejected marker using an unrelated storage failure, or report a false semantic
mismatch for an existing success marker.

Every validator must now explicitly classify rejection; it may return no verdict.
Only an explicit deterministic rejection may produce a negative marker. The production V2ApplyError match
is exhaustive, so a new typed service error requires a deliberate decision.
Untyped String validators are restricted to tests. Live validation returns a
local error before rejection persistence, and cold replay returns before any
accumulated success/rejection promotion or quarantine clearing. The existing
exact missing-sidecar outcome and deterministic rejection path remain intact.
Local failures use the existing I/O fail-stop/restart boundary, with their original
durable body/lifecycle authority retained; this adds no timer or second scheduler.

Two earlier flattening seams are closed as well. Lane planning preserves its
existing typed storage tag when entering Apply. Every error from reconstructing
committed DA indexes preserves the hydration context, including shard and receipt
cursors; direct candidate cursor errors remain deterministic body rejections.

## Exact evidence

Combined build54 passes for Core, Torii, test-network, Kagami and daemon library/
binary targets, plus `iroha_core_group_02` and `taira_consensus_contracts`, using
the same five-crate `scripts/cargo_fast.sh --stable-local-metadata --incremental
--jobs 4 -- test` selection with `--no-run --message-format=json --locked --offline`.
All 470 selected runtime controls pass: 417 Core and 53 Torii. Selection includes
all 89 Apply, 54 body-store, 79 lane-planner and 122 work-registry tests, 17 DA
hydration controls, 41 candidate tests, ten complete-input tests, two certified
gossip tests and three authenticated repair crash cuts, plus the retained Torii
ingress/receipt/retry controls. These are unit and handler tests, not a real-peer
network qualification.

Seven new regressions cover typed classification independent of diagnostic text;
actual damaged, previously authenticated Kura finality through both production
validation callbacks; all committed hydration categories versus direct candidate
cursor errors; typed planner input/storage distinction; exact body retry without
a negative marker; success and rejection quarantine retention; and the original
waiting lifecycle dispatch/wake authority through local failure and exact retry.
The real Kura-first fixture holds State at its pre-Apply generation/hash, damages
actual finality bytes, verifies unchanged body-store markers/catalogs and Kura
bytes across repeated local errors, then restores exact finality and verifies
successful same-body validation and original-marker promotion. It does not prove
the unfinished prepared-journal production cutover or automatic resource retry.

The full canonical source-binding gate reports zero diagnostics. All 14 selected
formal controls pass: twelve parser controls and two actual-source admission/
passive-recovery controls. New live/replay rejection-order requirements are also
checked by the compiled body-store source contract within the runtime selection.
Broader earlier formal counts are not attributed to this candidate.

Build/runtime captures cover 7,059 inputs; formal captures cover 8,137. Both now
explicitly include `crates/iroha_core/src/sumeragi/source_contracts_v1.txt`, which
is compiled by include_str! and was not part of the prior Rust/JSON/Python file
selection. All captured inputs, test binary hashes, HEAD and index remain
unchanged. Raw receipts are under ignored `dist/sumeragi-main-work/`:
`validation54.json`, `build54-source-{before,after}.json`,
`local-validation-controls54/summary.json` and
`formal-local-validation54/summary.json`.

Build52 passed compilation on unchanged inputs, but its runtime selection passed
466/470. Two older hydration fixtures bypassed the canonical Nexus/World setup;
the new damaged-finality fixture stopped before finality publication, and a
source-contract token did not match Rustfmt's line split. Build54 corrects only
those fixtures and the token, preserving all assertions and production semantics.
The failed52 receipt remains distinct from final54 evidence. Build53 subsequently
passed compilation and the canonical gate, but its confidential-receipt fixture
still lacked the privacy manifest required by the active SplitReplica policy.
That setup is corrected in build54 with a real privacy policy; receipt assertions
and production validation remain unchanged. Scoped Rust formatting,
retired-codec and diff guards pass. A broader formatting check reports existing
Torii part_5 fixture formatting outside these edits; its bytes are unchanged from
source51. No whole-workspace formatting pass is claimed.

## Open outcomes

The generic Validation diagnostic still has callers for temporary local Queue
retirement/capacity refusal. Queue pending ownership can require the very height
blocked by validation to commit. A typed wait alone cannot resolve that cycle; retirement needs shared
authenticated drain semantics and preservation of pending admitted work. Physical
lock contention can use its actual release owner, but releasing the successful
scan guard is not evidence that pending work drained. Fixed configured-capacity
refusal likewise has no release event. Treating these cases as storage restart
is not their final liveness design. This change does not reinterpret historical
invalid markers.
The first-release policy adds no migration or compatibility path.

Complete journal capture still needs early allocation admission, retained
State/Queue ownership through fresh/cache/reproposal/unfinished recovery, and
single-consume publication. Installation must cover COW and delayed reclamation;
existing body bytes and measured-size estimators are insufficient. Production
shared-lane cutover, mandatory metadata/evidence capacity, pool refill/startup
cost, geometry and participant durability, full workspace tests and one unchanged
four/seven-validator fault/restart/final-transaction campaign remain open.
No L1–L6 outcome is closed.

## Superseded root prose

The latest combined Core/Torii/test-network/Kagami/daemon build51 passes, as do all 286 selected runtime controls (233 Core, 53 Torii) on unchanged 7,058 Rust/Cargo inputs and unchanged test binaries. Signed RS16 capacity and live pre-receipt envelope checks remain enforced. The existing candidate assembler now fits optional evidence against exact proposal wire/chunk limits, alternates economic/evidence priority by height independently of view, and preserves mandatory penalties, pulse and retained input custody. An unfit evidence-only attempt defers without signing a pulse-only carrier; later ordinary work can proceed. Cold startup reauthenticates completed equivocation reports from their original durable lifecycle ledgers through verified Kura finality before ingress readiness. The final full source-binding gate and three affected controls pass on 8,136 unchanged inputs; the broader 138-control pass belongs to source49, before test-only fixture corrections. The [carrier/evidence record](docs/history/2026-09-19/carrier-evidence-custody.md) preserves exact scopes, corrected attempts and remaining limits. Full workspace tests, bounded mandatory metadata and individual evidence envelopes, pool refill and startup-cost qualification, live shared-lane/Validate-to-Apply publication, participant durability and unchanged four/seven-validator qualification remain open. No liveness goal is closed.

The next Sumeragi cutover must carry the implemented consuming State publisher and exact retained output owner through the complete production Validate-to-Apply path, including cached/recovered markers and voting; preserve one original execution per exact subject. The single I/O worker must retain a move-only prepared entry bound to its service's original State/Queue pair. Cache and reproposal reuse that entry; cold unfinished recovery rebuilds it once, while already-applied recovery repairs durable completion without executing again. Reserve complete capture capacity before detachment, and propagate resource/physical refusal as typed local deferral without a rejection marker. Keep original World/runtime journals, source groups, witness, result bytes, exact lease-bound checkpoint receipt and verified QC/Kura authority joined before visibility. Carry the pristine QueuePlan/NPoS control owner, authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks into the canonical validator while preserving State generation and complete carrier custody. Retain the implemented Queue cut and original Kura guards through pending geometry/Queue retirement and participant durability; bind them to the exact State/Queue pair from V2ApplyService and keep later State/component acquisition try-only. Bind complete mandatory framing and signed RS16 feasibility to admission before durable receipts; preserve authenticated startup refusal of local capacity below signed RS16 and the live pre-receipt native/publication envelope checks. Complete the guaranteed global admission opportunity: bound mandatory metadata across policy changes and each admitted evidence envelope. Preserve the exact optional-evidence fitter, height-based class opportunity and original-ledger cold restoration; finish bounded evidence-pool refill and startup scan cost. Autonomous anchors bind their original global height and cannot simply be trimmed into a later carrier; connect their reachable next action through the shared lane owner. The protocol input cap alone is insufficient. Finish bounded pre-vote descriptor/capacity admission, capture, file-handle and installation resources, and reachable typed deferral. Capacity scanning must precede geometry/sidecar acquisition; public geometry/GC/history wrappers cannot run under the held joint lease. Preserve original route-directory custody during retirement and account for witness staging on physical retries. Keep obsolete State observations as typed refreshes, including a finalized height advance, and preserve independent beacon and key-lifecycle behavior. Complete and qualify Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease coverage before opening the live Native header gate. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded history references, retaining exact source/finality/WSV joins and whole-bundle accounting. Defer physical GC behind release, snapshot/recovery pins and an instance-local deletion fence; qualify native Windows namespace durability separately. Connect the process-lived shared lane reducer to transport and canonical Decision application while retiring the old fresh-signing authority in one complete cutover. Preserve cross-route closure, immutable incarnation storage, drain/signing fences and historical native authority. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate and the full workspace before closing any liveness goal. Source-scoped evidence lives in [status](status.md); [superseded local plans](docs/history/2026-09-19/merge-source-publication-evidence.md) are preserved verbatim.
