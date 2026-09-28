# Native staking reconciliation — 2026-09-28

This work uses only `/Users/takemiyamakoto/soramitsudev/iroha`, branch
`optimizations`. The implementation/qualification goal remains active. No live
network or value-moving operation was performed.

The other merge owner still controls MAIN and the index. The staking review is
retained below `target/staking-validation/native-merge-review`; its before/after
captures and composition receipts are local review artifacts. Checkpoint 03
contains 266 changed paths, applies to its exact captured base, and has patch
SHA-256 `9ca723d48c5b1c24efb30acbbe1d274b0b366ba2b5f8de28140676bf225e99f5`.
Later native archive, strict scalar restore and SDK evidence changes are separate
dependent receipts, not evidence that checkpoint 03 was compiled.

## Scoped validation

| Captured cut | Evidence | Limitation |
| --- | --- | --- |
| Pre-merge Core staking fixture repair 11 | 182 passed, one failed; binary SHA-256 `f4e27017bfa2bcdcfafec75e7c9074da8a7b372a167c5ca1e046f1ab36fde40a` | Strict snapshot rejected omitted committed NPoS policy; exact fixture repair remains unrun |
| Native consensus work/control reconciliation | 361 passed, zero failed, two ignored; binary SHA-256 `fe3fc80d2785d7aba8bb005d26acff1243fd5437a660c7c5ae71a4473e9b9dc1` | Actual captured consensus component; heavy opt-ins and real networks not qualified |
| MV/Concread initial and writer backing | 513 passed, zero failed/ignored across MV 286, integration 170 and Concread 57; receipt SHA-256 `5bb7a11a56965c37b69d282d3601325bf5d1e546b4d422d3b321886c44cca1f7` | Actual captured components and cached real dependencies; no Core/workspace qualification |
| Fixed-memory Merkle root | 73 passed, zero failed/ignored; actual captured Crypto test binary SHA-256 `96962759fa60a3b2f31fd885da7d499a9759ff0e2e1927cf520bf451f1ca3daf` | Component evidence includes large/ragged parity; not an integrated Core check |
| Strict scalar snapshot parser | Three passed; canonical charged-Cell equality, negative/truncated inputs and 1,024 zero-allocation decode attempts | Exact parser component; aggregate State restoration remains unrun |
| Native evidence Python migration | 812 passed in 90.96 seconds; log SHA-256 `a08a31617906646651ccde3af09701364f6e548f523b1eabc366fa6cd5e14657` | Isolated earlier script overlay; its superseded lane semantics do not qualify the new lane layout or runtime |
| Kotlin/Java native evidence codec | Components compile with JDK 8 restriction; two codec runtime checks pass | Full fixture/finality and SDK qualification pending; Swift has syntax evidence only |

The original failed allocation census run is preserved. Its assertion omitted
one identity-mutex notification already funded by the production base. The
repair adds that exact 88-byte allocation to the expected sum; the exact demand,
refusal and retention assertions remain.

## Connected implementation under review

The native header is the sole signed control owner. Complete native journals,
real signed genesis plus H2 authentication, shared canonical R/context proofs,
all-seat frozen transitions and original Pasta receipts replace retired formats.
No old-context authority adapter or fallback decoder is authorized.

The mandatory context archive streams complete original pre-apply context values
into one original-pool buffer, verifies its equality with certified R, retains it
through publication retry and acknowledges only after durable publication.
Production opening retains the original Kura directory descriptor and rejects
a replaced pathname. Startup replay may reproduce a missing record only by reexecuting the certified
block and reproducing R. A conflicting record is rejected. New Core failure and
four-seat replay regressions are staged, not yet compiled or run.

The offline Kagami collector reads stopped original Kura plus that archive and
independently pinned genesis/network/chain inputs. Full native verification must
precede evidence publication. Genuine native fixture generation and replacement
of all grouped synthetic fixture assertions remain open.

The current lane specification and active node own lanes through
`lanes::LaneRunner`, `SumeragiLaneMerge` and the global execution overlay. The
old Native Decision/QueuePlan lane lifecycle is superseded. An integration audit
found that connecting its `PublishedNativeApply` or `v2_lane_driver` would create
a competing runtime. The uncompiled source/fixture stages for that integration
are preserved only as superseded review evidence and cannot qualify the release.
Their substantive execution, custody, refusal and source assertions must be
ported to the actual lane merge path before the old lifecycle is deleted.

The shared execution proof and archive are therefore being replaced directly
with `NativeLaneStateProof` in `R.native_lanes` and complete original
`SumeragiLaneState` projections. No old proof alias or fallback decoder is
permitted. The actual genesis execution now prepares its original projection
and publishes it after exact Kura/certificate equality but before State
visibility; the four-seat replay test includes both H1 and H2 archive recovery
and rejection of conflicting records. These changes have syntax evidence only.

The actual lane-state proof, strict current/predecessor snapshot cutover and
native route binding are now composed into the review. This includes an existing-only
archive reader that cannot create a missing directory, and exact State-source identity
on live read receipts. The global builder and merged suffix both use the committed
Sumeragi policy and pinned lane dataspace; retired Nexus routing cannot change the
execution coordinate. These changes and their focused tests remain uncompiled.

One shared historical lane verifier checks the exact pinned committee, original
predecessor, native quorum and reproduced admission result. Its new tests derive R
from the actual `LaneExecutor` and form real BLS signatures. Structural merge fixtures
with seeded lane QCs remain component-only evidence; they do not prove authenticity.
The Kagami corpus is being migrated to original executed and genuinely certified lane
frames and the same verifier. No new corpus or network pass is claimed.

The native Kura reader and off-chain State/Torii proof consumers are now composed.
Execution authority comes from an exact native quorum or the actual H2 genesis
anchor. Descriptor-bound slot metadata admits every prefix frame before body I/O
and decode, with separate aggregate-byte and source-height ceilings. The consumer's
exact target length remains mandatory even when a larger frame fits its source
budget. The reader uses original canonical frames and the shared native verifier;
there is no header-only or old-sidecar acceptance fallback. Complete decoded-graph
funding, old proof-fixture migration, deterministic instruction-history provenance,
and retirement of the old V2 Kura/replay/Apply caller chain remain open.

An expansion now retains its exact borrowed State and stable publication through
its receiving block boundary. A foreign or changed State is a local retry, retains
the original proposal, and emits no peer rejection. The sole native signed-genesis
path installs both schedule and lane state, then checks the signed execution/Nexus
policy commitments before sealing outputs. Unpublished builders may use the typed
policy-mismatch result to derive corrected commitments; their final newly signed
genesis must pass the same strict validator. Those tests are staged, not run.

The original execution recorder identity is retained from pristine construction
through first witness capture. Old State replay/publication authority bypasses and
46 superseded carrier-owner files were removed in a source checkpoint. A later
source cut removes QueuePlan enqueue-time admission from the actual output path:
merged inputs are filtered and validated at G block time as the lane specification
requires. Genuine expired-merged/no-fee behavior and the broad old-fixture migration
still require runtime verification.

The earlier physical census found 30 of 31 World Cells and 275 ordinary Storage
fields using default outer allocation mode. That is historical scoped evidence,
not a fresh count after owner removal. Some payloads retain nested charges; complete
production funding is still open. Permanent DA/confidential indexes, cursor maps,
receipt projections and original retained generations remain unqualified.

Real 4→7→4 transitions, noncommittee-sized candidate pools, withheld fresh keys,
missing target custody, Parliament pulses, authenticated loss/replay/all-seat
restart, slashing, rewards, complete withdrawal and maintained formal/DA/liveness/
workspace/SDK gates must pass on one unchanged candidate. Syntax checks and
isolated component passes do not close those outcomes. XOR remains the immutable
network-authenticated asset; disposable genesis funding does not create a second
production token or imply mainnet monetary value.

## September 29 integration checkpoint

The separate merge owner finished resolving and staging the merge without a commit.
Incoming relation-count instrumentation is retained in the shared native prefix
verifier; it does not restore the obsolete signature-only verification path.
The current native evidence collector has 153 passing Python controls, including
the 65,536-row bound. The earlier 812-test overlay used superseded protocol DTOs
and is not evidence for this native candidate. Native fixture-generator sources
now execute the actual signed genesis, lane admission and native quorum; genuine
regenerated captures and integrated Rust/SDK/network qualification remain open.

Six shared Core scheduling/overlay consumers now retain the real native test
execution through publication, preserving nine named test assertions. Remaining
old shared fixture callers and retired State/SDK source families are still under
migration; syntax checks do not establish their compiler closure.
