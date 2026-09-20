# Carrier scheduling and evidence custody — 2026-09-19

Work remained in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`, at
HEAD `87e0e45d17cc726e1afc9819746a652b8f694f20`. The index remained unchanged.
No branch, worktree, commit, compatibility path or sibling documentation edit was
created.

## Implemented boundaries

The existing candidate assembler remains the sole selection owner. Even proposal
heights give economic work first opportunity; odd heights give optional evidence
first opportunity. View changes cannot change this priority. A binary search fits
the largest canonical evidence prefix against the exact unsigned proposal wire
and signed RS16 chunk geometry before signing. Mandatory penalties, pulse,
autonomous anchors and other framing remain in the sizing base. Each ordinary/
admission retry starts from the original effects, and deferred inputs and proofs
retain their original custody. The economic merge path suppresses optional evidence
only after finding an actual eligible execution batch and never suppresses required
penalties. A proof which cannot fit by itself yields typed evidence-envelope
deferral rather than a signed pulse-only carrier or runner failure; a subsequent
ordinary arrival can proceed at the same height.

Completed lifecycle equivocation reports previously disappeared from the pending
candidate pool after a cold restart: their terminal ledger rows were intentionally
excluded from output reconstruction, while the new State had an empty pool. The
passive reader now extracts only the original wire proof of a completed Advanced
EquivocationReport from a context-bound, completely validated existing ledger.
It neither creates nor cleans storage nor restores a Ready/output capability.
Exact descriptor-bound file reads reject corrupt, foreign and substituted links.
Cold startup discovers contexts through cryptographically verified Kura finality,
then applies the unchanged signature, network, duplicate, committed and horizon
filters before ingress readiness. The active authenticated context uses the same
restoration before worker I/O starts.

## Validation and corrected attempts

Combined build51 passes for Core, Torii, test-network, Kagami and daemon library/
binary targets, plus `iroha_core_group_02` and `taira_consensus_contracts`:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test \
  -p iroha_core -p iroha_torii -p iroha_test_network -p iroha_kagami -p irohad \
  --lib --bins --test iroha_core_group_02 --test taira_consensus_contracts \
  --no-run --message-format=json --locked --offline
```

All 286 selected runtime controls pass: 233 Core and 53 Torii. This includes the
three new exact-carrier tests, four new completed-evidence cold recovery tests,
all 136 runnable lifecycle ledger controls, all 32 evidence controls, and the
previous capacity/ingress, complete-input, gossip and authenticated repair crash
cuts. One explicitly ignored operator inspection tool requires an incident frame
and is reported separately; it is not counted as a runnable test or a pass.
All 7,058 captured Rust/Cargo inputs and Core/Torii binary hashes were unchanged.

The cold positive uses genuine four-BLS votes, actual lifecycle completion and
terminal durability, a real Kura body/finality writer, a dropped old State and a
new State, then the actual follower evidence-admission validator. Its structural
executed-body/header helpers are a fixture boundary, not daemon/genesis/Apply
qualification. Candidate scheduling controls prove signed source custody, exact
wire boundaries, same-height view stability and later ordinary opportunity; their
earlier evidence context is not installed as follower history. Do not attribute
follower admission qualification to those scheduling-only controls.

Preflight47 found private fixture-field access; preflight47b found an incorrectly
typed deferral. Build48 failed three fixture proof/adapter type mismatches.
Build49 passed compilation and 282/287 observer entries: four new cold tests had
an invalid chain-ID fixture, and the observer had incorrectly counted the ignored
incident tool as failed. Build50 passed compilation and 283/286 runnable tests:
three cold fixtures exposed signing-preimage construction and noncontiguous
missing-finality setup errors. Build51 corrects those fixtures using the actual
unsigned Vote preimage and a contiguous height-one negative; all original
assertions remain. Build48–51 each retained their captured Rust inputs unchanged
during compilation. Mutable preflights establish no frozen-candidate result.

Frozen source49 passed 138 focused formal controls (55 capacity, 47 passive
recovery, 22 inventory, 12 parser and two original-review controls) and the full
canonical source-binding gate on 8,136 unchanged inputs. Only cold-test fixture
corrections followed. Final source51 passes a fresh full canonical gate with zero
diagnostics and three affected current-source/real-cold-fixture controls on 8,136
unchanged inputs, HEAD and index. The broader count remains attributed to49.
The separate full formal50 rerun passed all 138 controls and the canonical gate,
but overlapped the declared final correction in the one cold-test file. Its
before/after receipt records that exact drift; it is not an unchanged-candidate
result. Scoped Rustfmt, the retired-codec guard, `git diff --check` and historical
archive verification also pass (64,736 records and 67,311 occurrences).

Raw receipts remain under ignored `dist/sumeragi-main-work/`:
`validation51.json`, `build51-source-{before,after}.json`,
`carrier-evidence-controls51/summary.json`, `formal-carrier51/summary.json`,
`formal-carrier49/summary.json` and `formal-recovery49/summary.json`.
The canonical gate used the local validation virtualenv to run
`scripts/formal/check_sumeragi_v2_multilane_models.py --root /Users/takemiyamakoto/dev/iroha`.

## Remaining outcomes

Alternating priority is a selection rule, not a proof that every class can fit
under all mandatory metadata. Required policies, pulse and penalties need a
bounded coexistence contract across policy changes; an individually unfit first
proof can still block a later prefix. Autonomous anchors bind their original
global height: trimming them for a later global carrier would lose valid custody.
Their solution belongs to the production shared-lane cutover.

Recovery retains the original durable ledger, but the bounded pending pool can
decline excess proofs; no refill owner has been added. Startup scans the applied
evidence horizon through verified finality. The default horizon may read 7,200
artifacts and a zero/unlimited horizon scans the full applied chain once. This
does not establish bounded wall-clock startup. A future authenticated report index
or cursor must retain original-ledger membership; directory names cannot grant it.
Ready/Cancelled report rows are outside this completed-report restoration.

Production Validate currently discards prepared execution and Apply executes it
again; body-store cache and reproposal branches return cloneable receipts without
executing the validator. The next connected change must retain one move-only
prepared entry in the single I/O worker, bound to V2ApplyService's original State/
Queue pair. It must reserve complete capture capacity before detachment and use
typed local resource/physical deferral without writing deterministic rejection
markers. Cold unfinished recovery rebuilds journals once under the new process
owner; already-applied recovery repairs completion without execution. Current
terminal publication still refuses nonidentity geometry, pending autoscale and
participant-durability obligations. A target field alone would remain unused.

Full workspace tests and one unchanged real four/seven-validator fault/restart/
final-transaction campaign remain required. No L1–L6 outcome is closed.

## Superseded root prose

The latest combined Core/Torii/test-network/Kagami/daemon build46 passes, as do all 115 selected runtime controls (62 Core, 53 Torii) on unchanged 7,058 Rust/Cargo inputs and unchanged test binaries. Authenticated startup now refuses local body capacity below the signed RS16 envelope; candidate selection uses that signed limit. Torii checks active recovered capacity, worst-quorum native input size and actual publication frame limits before issuing journal receipts, retaining the request-memory owner during sizing. The formerly failing 800 KiB admission assembles with supported local capacity; missing capacity and native overflow leave the journal unchanged, and capacity loss after quorum remains indeterminate. All 77 focused formal controls and the canonical full source-binding gate pass on 8,136 unchanged inputs. The [capacity record](docs/history/2026-09-19/admission-capacity.md) preserves exact scopes and the two corrected test-compilation attempts. Build43's 542 runtime passes belong to its earlier source epoch. Full workspace tests, guaranteed global admission opportunities under mandatory metadata, live shared-lane/Validate-to-Apply publication, participant durability and unchanged four/seven-validator qualification remain open. Native DA/pin/SCCP execution and broader NPoS controls remain separate outcomes; no liveness goal is closed.

The next Sumeragi cutover must carry the implemented consuming State publisher and exact retained output owner through the complete production Validate-to-Apply path, including cached/recovered markers and voting; preserve one original execution per exact subject. Keep original World/runtime journals, source groups, witness, result bytes, exact lease-bound checkpoint receipt and verified QC/Kura authority joined before visibility. Carry the pristine QueuePlan/NPoS control owner, authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks into the canonical validator while preserving State generation and complete carrier custody. Retain the implemented Queue cut and original Kura guards through pending geometry/Queue retirement and participant durability; bind them to the exact State/Queue pair from V2ApplyService and keep later State/component acquisition try-only. Bind complete mandatory framing and signed RS16 feasibility to admission before durable receipts; preserve authenticated startup refusal of local capacity below signed RS16 and the live pre-receipt native/publication envelope checks. Complete the guaranteed global admission opportunity: bound mandatory metadata across policy changes and defer optional evidence/autonomous anchors without losing durable custody. The protocol input cap alone is insufficient. Finish bounded pre-vote descriptor/capacity admission, capture, file-handle and installation resources, and reachable typed deferral. Capacity scanning must precede geometry/sidecar acquisition; public geometry/GC/history wrappers cannot run under the held joint lease. Preserve original route-directory custody during retirement and account for witness staging on physical retries. Keep obsolete State observations as typed refreshes, including a finalized height advance, and preserve independent beacon and key-lifecycle behavior. Complete and qualify Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease coverage before opening the live Native header gate. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded history references, retaining exact source/finality/WSV joins and whole-bundle accounting. Defer physical GC behind release, snapshot/recovery pins and an instance-local deletion fence; qualify native Windows namespace durability separately. Connect the process-lived shared lane reducer to transport and canonical Decision application while retiring the old fresh-signing authority in one complete cutover. Preserve cross-route closure, immutable incarnation storage, drain/signing fences and historical native authority. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate and the full workspace before closing any liveness goal. Source-scoped evidence lives in [status](status.md); [superseded local plans](docs/history/2026-09-19/merge-source-publication-evidence.md) are preserved verbatim.
