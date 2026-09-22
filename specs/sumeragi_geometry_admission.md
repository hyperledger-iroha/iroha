# Geometry admission and immutable lane storage

Selected first-release design, 2026-09-18. The isolated candidate separates
canonical-chain storage from lane aliases. Immutable lane incarnations and Native
carrier bundles remain unimplemented and unqualified. The consuming State publisher
and retained production-adapter candidate now exist; live integration and network
qualification remain open. Their scoped memory admission is described below and
does not complete this storage cutover. Replace the layout directly; do not add a
compatibility resolver or alternate production path.

## Decision and cause

Use one stable canonical-chain namespace and immutable lane-incarnation
namespaces. Publish authenticated catalog references when geometry changes;
reclaim physical storage separately, after proving it is no longer referenced.
Human aliases affect catalog and metrics metadata, never physical identity.

Today `LaneConfigEntry::from_metadata` derives physical paths from LaneId and
alias. `Kura::build_geometry_operations` moves namespaces for relabel, replacement
and retirement. The fixed canonical namespace now prevents these moves from
relocating `active_blocks_dir` or its global merge ledger. Lane admission
consequently scans, repairs and synchronizes mutable evidence across routes.
Apply repeats local admission after finality and can refuse on local Queue
contents. Physical relocation couples a decided catalog change to local
maintenance and unrelated publishers.

No inspected consensus rule requires physical rename or deletion at logical
retirement. Do not implement the previously proposed consensus-spanning global
evidence freeze to preserve that coupling. Physical evidence scans remain
necessary for reclamation and authenticating historical obligations.

## Existing implementation boundaries

The canonical owner uses `blocks/canonical` and `merge_ledger/canonical.log`.
Each lane, including the primary lane, has a distinct sidecar namespace. Canonical
paths cannot be claimed by a geometry operation, including an ancestor or a
child of those paths. Relabel/recovery never retarget canonical handles. A bound
catalog with missing canonical storage fails closed; startup does not fall back
to an alias namespace or initialize a replacement empty chain. Existing block
and merge budget roots charge canonical files once.

Canonical open preflight checks its exact files, both immediate parents and the
store root. It does not repeatedly traverse unrelated retired lane evidence.
The separate configured-lane owner retains its existing tree checks. Canonical
stage recovery precedes required-component checks so an authenticated interrupted
rewrite remains recoverable. These boundaries do not remove the existing
canonical-chain serialization around geometry publication. Core unit-test
compilation passed for the initial split; full Kura diagnostics and the bounded
preflight refinement are still being qualified.

`StateBlock::prepare_carrier_geometry` in
[`carrier_geometry_preparation.rs`](../crates/iroha_core/src/state/carrier_geometry_preparation.rs)
retains actual MV predecessor/successor catalogs, manifests, lineage, activation,
the pending lifecycle transition and certified drain frontiers.
`PreparedCarrier::prepare_journals` retains that object and original State
journals; it grants no storage or publication authority.

In [`lane_geometry.rs`](../crates/iroha_core/src/kura/lane_geometry.rs), the
existing transition holds prune, canonical-chain, geometry and sidecar locks,
performs maintenance/admission, then moves paths. Its prepared journal retains
exact operations and one bounded canonical phase encoding. It still requires
the old lock scope and is not the new reference-publication owner.

Explicit retirement maintenance preserves certified-frontier recovery,
authenticated compaction and all seven progress-pair repairs. Native and
historical recovery evidence have bounded observers and consuming durability
attesters. Observation preserves exact file identities, hashes and complete
namespaces, including absence; attestation rejects substitutions. Current callers
retain their original locks. The remaining autonomous evidence reader still
synchronizes files. These separations support maintenance/GC; they do not
qualify pure aggregate admission or a consensus reservation.

The legacy Apply path captures archives after finality, repeats retirement checks
and uses the local Queue veto. The retained candidate captures original archive
owners and consumes the exact executed carrier through its State/Kura/Queue pair.
Keep raw State publication guards until that consuming path replaces its callers
and is qualified. This does not remove the remaining immutable-storage and
historical-completion obligations below; removing guards alone is unsafe.

## Authority and ownership

| Owner | Required authority and responsibility |
| --- | --- |
| Canonical store | Stable network-scoped root; exact canonical wire, block hash, QC, finality and application metadata. A lane alias or incarnation change cannot move it. |
| Active admission | Authenticated current State catalog plus exact network/route/incarnation/activation. Only this owner admits new lane work or signing. |
| Prepared carrier | Actual predecessor and successor references, original State journals, exact admitted new-instance provisioning/reference bytes, archive/resource reservations and carrier identity. |
| Historical completion | Exact originally admitted instance plus authenticated work and canonical application authority. Owns retained inputs, publication targets and retry state through completion/restart. Current LaneId lookup is not authority. |
| Physical GC | Snapshot-proven reachability, authenticated terminal/release evidence, capacity accounting and an instance-local deletion fence. It cannot alter a decided catalog verdict. |

Canonical drain remains mandatory. Preserve the exact signed high-water,
incarnation-bound drain frontier and permanent signing fence. Local pending
work can delay a drain vote; after authenticated drain and carrier decision,
another local Queue snapshot cannot add a canonical validity condition.

Cross-route Native work requires a separate exact join. A sibling coordinator
can have admitted participant work on the retiring lane; the existing scanner
correctly distinguishes that from a retiring coordinator's own drained work.
Before removing its veto, prove that every such obligation either has an
authenticated terminal outcome or retains executable historical completion
authority. Keeping bytes alone is insufficient. New admission to a retired
incarnation stays prohibited.

## Native completion cutover

The current Native evidence planner authenticates the canonical carrier,
QC-bound execution/manifest root and every exact route receipt. Post-WSV repair
also joins the commit manifest and WSV checkpoint. Preserve those checks.

Replace mandatory per-route application manifest/receipt/latest files with one
immutable authenticated evidence bundle per exact canonical Native carrier,
stored in the stable canonical namespace. A bounded route/incarnation history
catalog references that bundle. These artifacts contain deterministic projections
of the canonical execution and verified finality; they add no separate application
fact. The current snapshot blocks producers on missing derived per-route files
even when the applied World frontier matches. Remove that duplicate-state failure
through the complete representation cutover, not by ignoring the missing files.

Select a complete proof bundle rather than references that require retaining whole
result-bearing bodies forever. Current authenticated hash-only bootstrap and
body eviction can omit the carrier bytes while retaining Native evidence. Compact
merge carriers also require their exact associated `MergeLedgerEntry`; the
carrier and QC alone cannot reconstruct that execution. Before releasing either
source, derive, persist, read back and authenticate the entire bundle from the
actual body, associated merge batch and verified finality. Preserve every leaf,
proof and receipt-preimage check, canonical leaf order/count, executed-wire hash,
source/result/proposal/settlement joins and exact checkpoint/commit-manifest
application authority. A partial bundle or latest pointer cannot release pins.

One carrier publication/recovery owner installs the bundle and its complete
catalog references. Do not recreate independent per-route completion flags.
Finality before the WSV join remains pending; complete authenticated application
must not require repairing another derived copy before production resumes.
Per-route inode identity is local mutation protection, not a protocol commitment.
The new immutable store must enforce namespace/object integrity at its own bounded
boundary; current sidecar guards remain until this complete cutover exists.

Ordinary predecessor reads, history observation and restart discovery use full
original instance identity, never a historical boolean or current LaneId fallback.
Preserve the complete Native settlement chain independently of ordinary lane
interleaving. Current State discovery only enumerates active lanes; authenticated
historical catalog references must replace that limitation coherently.

Publication indexes pin exact carrier bodies across non-tip restart.
Reconstruct instance pins from authenticated manifests, not directory names.
Sidecar repair for an already applied canonical carrier does not authorize
execution of a pending participant source with no application carrier. That
pending-work admission/closure join remains a distinct implementation gate.

## Publication, resources and recovery

1. Before decision, capture deterministic catalog/drain semantics and admit
   bounded new-instance provisioning and exact reference writes. Retain original
   predecessor and successor roots, including same-height replacement/undo.
   Preparation must not depend on physical retirement or a GC scan.
2. Persist canonical body/finality and consume exact QC/Kura authorization to
   publish retained State, archive and instance references. Do not recompute
   State, rerun local work admission or rename/delete historical namespaces.
3. A late I/O failure retains the original plan and retry owner. Restart derives
   authority from authenticated durable intent, carrier and references; a
   process token or directory name is not authority. Until publication finishes,
   recovery keeps both original roots reachable.
4. Release obsolete references only through the proved undo/recovery/retention
   policy. GC may then authenticate evidence and reclaim a bounded instance
   under a local deletion fence. Concurrent historical readers/writers pin that
   exact instance; reference admission and deletion cannot race.

Current production-adapter admission covers bounded candidate descriptor slots,
two concrete World journal shell sets, the retained effects `Box` and the candidate
phase `Box`, charged to a finite shared pool. The original shell/effects reservation
moves with the actual executed carrier through capture, decision binding, physical
refusal, publication and final destruction. Descriptor and phase charges follow
their actual lifetimes. There are no artificial decision-binding or installation
admission callbacks. Publication still requires exact source/execution/finality
and the original State/Kura/Queue owners; a local retry never reruns execution.

Complete process-memory accounting remains an outstanding goal. Nested map/value
and execution/event allocations, current/undo COW, tiered snapshots, geometry and
archive projections, wire encoding and decoding are not prepaid by that shell
pool. Account for them at their actual allocation and release boundaries, including
publication peaks and delayed reclamation, before claiming a complete bound.
Neither the scoped pool nor the consuming facade establishes live activation or
network qualification.

The immutable-store capacity goal is separate: admit all retained instance bytes,
candidate directories, temporary publication peaks and recovery records under
enforced capacity. Preserve separate certified,
post-WSV and Native reservations without double charging. Competing writers
cannot spend reserved capacity; abandoned candidates cannot leave unbounded
orphans. Resource refusal before decision is typed local deferral with a bounded
retry/wakeup owner, never durable invalid-body evidence. GC corruption or missing
evidence delays reclamation; exhausted capacity may delay later admission.

No raw State/Kura lock may span consensus waiting or an awaited worker that
requires it. Local deferral must propagate through validation workers and
BodyStore without becoming `Rejected` or generic fatal recovery. Cancellation,
release and restart retain or wake the actual owner without busy polling.

Directory descriptors are admitted resources too. A Native manifest can name
1,024 routes. Prototype530 was withdrawn because it captured every route's
ancestor chain after finality with no portable descriptor guarantee. The selected
carrier bundle avoids that mandatory per-route handle set. Reserve the actual
bounded store resources needed by the chosen publication plan before voting;
retaining more handles in the old design is not the selected fix. Reopening from
cached device/inode values is not equivalent exact-object protection. Do not
compensate with a smaller local route cap or process-limit changes.

Charge an exact shared carrier bundle once, but charge all of its bytes while
any route, retired instance, snapshot, undo root or incomplete publication refers
to it. One slow route can pin the whole bundle after every other reference has
released. Admission includes this amplification, complete old/new roots and
temporary publication peaks. GC releases the bundle only after its last exact
owner, never by subtracting fractional per-route bytes.

The scalar validation path computes the participant manifest in
`PreparedCarrier::prepare`, returns an execution commitment and drops the owner;
its marker/reproposal shortcuts are insufficient publication authority. The retained
production-adapter candidate replaces that loss with actual carrier custody in
bounded slots, including capturing, validated, decided and checkpointed phases.
Its consuming facade returns the same carrier and phase on local refusal. The
worker/result handoff, cached validation, recovered markers and finality consumer
must all use that original owner before votes are authorized, preserving the exact
manifest, candidate and predecessor. An equal execution hash, a reservation field
on a discarded object or a second execution cannot replace this join.
The local Queue veto now retains its original Validate dispatch, acknowledgement
and exact route-release observation without writing a `Rejected` marker. Storage
and drain-observation failures retain typed local recovery provenance through
block validation and emit no rejection event. The exact sample survives an
autoscale retry; evaluation is complete only after the fallible lifecycle step
succeeds. These paths pass the historical 76-test DPN development build19 selection;
that result does not qualify the new retained adapter or consuming facade. Complete
runtime handoff, original Apply settlement, cold recovery and network qualification
remain required. Full process-memory admission is a separate outstanding goal;
the current ownership path supplies only the scoped allocation charges above.

## Ordered implementation and acceptance gates

TODO: complete this cutover before removing the old physical admission path.

1. Finish qualification of the implemented canonical-chain/lane storage split.
   Replace remaining alias-derived lane physical identity with immutable
   authenticated instance paths. Alias changes then leave both canonical and
   lane bytes at identical physical locations.
2. Replace per-route Native application publication with the complete immutable
   carrier bundle and authenticated history references. Test missing body/merge
   sources before/after bundle installation, exact proof count/index, non-tip
   recovery, ordinary interleaving, reused LaneId and slow/retired-route bundle
   pins. Retain exact historical unmerged-work authority and prove cross-route
   closure separately; fresh admission/signing stays active-only.
3. Replace move operations with bounded authenticated reference/provisioning
   plans. Test predecessor/successor substitution, one-byte-over capacity refusal
   before effects, append, replacement/undo and all crash cuts.
4. Connect the consuming State/Apply publisher and typed retry owner. Two nodes
   with different local queues publish the same decided catalog. Historical work
   finishes without permitting new active work on the retired instance.
5. Move physical retirement checks to bounded GC. Prove snapshot and recovery
   pins, interrupted publication, cross-route work and an in-flight historical
   writer prevent deletion; after exact release, reclamation completes.
6. Update source bindings alongside owners and their mutation controls. Qualify
   one unchanged candidate with four/seven-validator loss, reordering,
   backpressure, leader failure, restart and final-transaction tests. Preserve
   exact 3f+1 committees/2f+1 votes, mandatory signed RS16 and progress without
   an empty-block dependency.

Observer tests and earlier regression runs are scoped evidence, not completion
of these gates. Full formal, consuming Apply and network qualification remain open.
