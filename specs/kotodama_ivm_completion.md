# Kotodama and IVM completion

G1–G8 remain active and incomplete. The table below defines their outcomes;
component implementation and historical test results do not establish release
qualification.

## Goals and dependency order

| Goal | Required outcome | Current work |
| --- | --- | --- |
| G1: language and ABI | Caller-owned argument/result word tables for every compiled function; 64 KiB / 8,192-word limit; initialized-count, ownership, alignment and lifetime checks; full-width integer helpers and structural equality. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G2: authenticated State | One State-owned commitment over every authoritative persisted execution table; bounded inclusion, absence and complete range witnesses; atomic finalized publication and persistence/recovery custody. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G3: private execution | One complete native STARK relation and `IvmExecutionStatementV1`; normalized intent, exact public gas, complete reads/effects, private-trace masking, full typed valcom constraints and native local proving. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G4: anchored AXT | One `AxtAnchoredSpendV1` path binding finalized successful execution and exact transfer occurrence, fresh issuer authorization, atomic durable nonce/budget/effects, complete FASTPQ state transitions. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G5: memory ownership | Allocation-lifetime charges shared once across prepared artifacts, analyses, operations, templates and borrowed/nested VMs; aggregate 64 MiB retention; separate admitted active reservations; zero retention; observable live/borrowed/evicted/peak usage. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G6: automatic acceleration | Standard target-appropriate SIMD/Metal/CUDA, runtime-loaded driver, process owner, per-device/kernel qualification, public cost selection, unchanged-input fallback/quarantine, ten reproducible signed embedded PTX families. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G7: Musubi publication | Daemon-owned authenticated private TLS runner, finalized recovery, software signing, provider coordination/readback, durable journal/clock and filesystem custody, configuration through `iroha_config`. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |
| G8: candidate qualification | Regenerate ABI/schema/gas/SDK fixtures together; qualify one unchanged candidate across mandatory language/proof/state/AXT/memory/hardware/native/SDK/network/release checks. | Open; complete the stated outcome and the integration boundaries below on one qualified candidate. |

Component owners are compiler/IVM/Core/SDKs for G1; State/Kura/consensus for G2;
the native prover, verifier and transactional execution owner for G3;
AXT/FASTPQ/Core for G4; VM/cache/allocation owners for G5; acceleration/build/CI
for G6; daemon/config/provider services for G7; and release/native/SDK/network
qualification for G8.

Freeze the canonical ABI, numeric semantics, proof statement, state commitment and
AXT shapes first. Language, memory, hardware and Musubi implementations can then
proceed independently. Complete finalized state publication/recovery before
integrating private proofs and AXT. Regenerate and qualify a single candidate
only after these paths are coherent.

## Fixed invariants

- `r10/r11` carry argument-table address/count and `r12/r13` carry result-table
  address/capacity. Returns carry result address/exact initialized count in
  `r10/r11`. Caller-owned returned storage outlives the callee.
- Current `Secret<T>` information-flow restrictions remain. General secret
  arithmetic/branching is outside scope. Raw witnesses stay local and never
  enter transactions or Torii requests.
- Hardware changes speed only. Results, errors, gas, state, event order,
  commitments and verification decisions must agree across devices. Test
  entropy plus an identical witness yields identical proof bytes; production
  randomness is fresh.
- “Optimal” means the fastest qualified operation/workload path including
  transfer and launch cost. Selection uses only public workload geometry.
- Cache pressure changes local scheduling or retention, never transaction
  validity or gas. Eviction never releases a charge still owned by a borrower.
- Public ledger replacement is a separate deployment operation. Initial network
  evidence uses disposable networks with at least four validators and mandatory
  signed RS16 DA/RBC.

## Next integration boundaries

G2 must classify every World, State, trigger and durable-history owner as canonical
authority, checked derivation, authenticated history or local-only policy. The
current World baseline commits secondary indexes independently and omits State
owners; it is not the final commitment. Semantic configuration consumed by
execution must be authenticated through agreed parameters or context. A canonical
table identity must bind its key/value schema and Norito layout, not just a Rust
field name. Snapshot JSON hashes and touched-write roots are not substitutes.

The complete root and its exact predecessor must travel with prepared State
journals and publish under the same State generation as World, runtime and replay
membership. Recovery must authenticate that owner before exposing State. Current
hash-key Merkle lookup can prove bounded inclusion/absence, but cannot prove raw
key range completeness. An ordered derived commitment must authenticate range
boundaries, interior rows, tombstones and continuation against the same canonical
rows. Preserve the distinction between execution-prefix and finalized-State
commitments to avoid a header/root/finality cycle; table membership alone grants
no permission to disclose private rows.

The Kagemusha registry freeze cannot be lifted with a direct
`CanEnactGovernance` check: that permission covers enactment of an approved
referendum. The certified Parliament set now has one canonical initial
signer-policy proposal with an exact empty predecessor, effect preimage and
head compare-and-set. Its due-certificate reducer moves a one-use, fixed-size
authorization through transaction apply, and State rechecks the complete
certificate digest and resulting registry before publication. Unrelated writes
remain rejected; governed release install, activation, retirement and runtime
reload are separate unfinished transitions.

G3 retirement now removes the old four-hash/16-column binding schema, Halo2
IVM registration and key generator, dedicated native STARK binding relation,
Torii preparation/proving jobs, and SDK/CLI callers. The generic proof registry
rejects reserved IVM identities. Focused Core/Torii test builds and regenerated
OpenAPI provenance are still pending on the changing source tree.
The V1 statement model's `IvmCompleteStateRootClaimV1` is intentionally separate
from the current selected-table leaf/subset roots. Its constructor and digest
authenticate nothing on their own; only a Core-owned finalized State/Kura/QC/DA
handle can promote a root claim to an execution anchor. The access, return,
effect and event hashes are public claims until the complete interpreter/STARK
relation constrains their exact canonical contents and Core checks every current
dependency before transactional application.
Retain reserved-name rejection at generic proof-registration and OPEN_VERIFY
boundaries so removing the special binding circuit cannot re-admit it through a
generic digest circuit. The fail-closed admission is not evidence for private execution or finalized
full-state proofs; public preparation needs a new admissible V1 response shape.

G5 production activation still requires funded fallible remaining trace/debug and
shared-host dispatcher allocations; scheduler graph/result/channel/pool allocations;
memory-reset scratch and generic `Memory::Clone` allocations; and fully prepaid
active and nested host execution. The protected return stack is now reserved before
child-call gas, and call-frame bitmap and vector-slot growth is prepared before
table-validation gas. Inactive runtime-template copies no longer duplicate spare
frame capacity. These bounded cuts do not fund all frame and scratch owners.
Local diagnostic step/access recorders and the optional initial image now use
one checked parent-funded plan with child partitions; four new and 20 existing
recorder tests pass. This does not fund production VM or host allocations.
Parent-credit shortage
must not wait while holding parent funding. Configuration changes must retain the
original pool while allocations survive, and cache locks must be released before
synchronous allocation-release notifications can reenter the scheduler. Core exports measured resident, reclaimable, borrowed, evicted-but-live and peak
reservation gauges alongside retained, active and unmeasured-owner totals. Composite
retained-owner classification and funding missing active/scratch owners remain open;
these gauges do not represent complete process RSS. The new
pool and buffer APIs are not evidence that those production paths are funded.

The local physical runner is an Apple M1 Ultra with 128 GiB memory. Exact-candidate
M1 Ultra calibration remains unrun. Local required Metal kernel parity passes;
Graviton3, CUDA parity, mixed-hardware
four-validator execution and publication qualification still need their required
runners and service custody.

TODO: Freeze and record one complete candidate, regenerate all native/SDK artifacts,
and replace each open gate with its actual evidence. Physical runners, provenance
signing and deployment credentials are required inputs. Do not infer release
readiness from local components or represent skipped/missing execution as a pass.
