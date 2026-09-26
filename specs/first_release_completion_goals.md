# First-release completion goals

Set: 2026-09-20. Overall goal: **Active**. This record coordinates the accepted
privacy/ZK, SoraFS V1, and multilane implementation and qualification plan.
Starting checkout: `f11eed2d6c7113163295e65703d9c206a5ce07d9` (`optimizations`).
Required working directory: `/Users/takemiyamakoto/devstuff/iroha`.
Required branch: `optimizations`. Perform all further implementation and
validation here; do not create or use another checkout, worktree or branch.
The [September 23 merge transition](../docs/history/2026-09-23/optimizations-merge-transition.md)
records the concurrent source reconciliation; its unsigned merge commit is not
a frozen release candidate.
The [September 25 integration checkpoint](../docs/history/2026-09-25/first-release-integration-checkpoint.md)
records the current ABI-24 source cut, typed fixture recapture and scoped
validation without closing any candidate release gate.
The [integration checkpoint](../docs/history/2026-09-20/first-release-integration.md)
records preservation, implementation boundaries and scoped validation failures.
The [September 21 runtime checkpoint](../docs/history/2026-09-21/runtime-and-governance-integration.md)
and [SDK response checkpoint](../docs/history/2026-09-21/js-governance-response-validation.md)
record subsequent integrated checks. The [native40 migration record](../docs/history/2026-09-21/mkhe-native40-migration.md)
separates completed evidence-identity hardening from unresolved production work.
The [native40 source-gap audit](../docs/history/2026-09-23/mkhe-native40-source-gap-audit.md)
pins the missing governed 40-limb producer and the current qPCS work bound;
neither source migration nor production composite admission has completed.
The [qPCS geometry review](../docs/history/2026-09-23/mkhe-native40-qpcs-geometry-review.md)
reconciles the repeated-payload diagnostic with the larger full-tree charge and
records a nonconforming research hypothesis, without work or soundness
qualification under the fixed six-lane contract.
The [indexed-leaf and node work floor](../docs/history/2026-09-23/mkhe-qpcs-index-node-six-lane-floor.md)
shows that the present qPCS tree exceeds the 128-billion-work limit even with
payload hashing excluded; it is a lower bound for that construction, not a
qualified redesign.
The [qPCS Merkle-opening work preflight](../docs/history/2026-09-24/mkhe-f06-qpcs-merkle-opening-work-preflight.md)
passes 3 focused budget/geometry checks and 7 existing tree controls, but the
original source-session budget still does not reach a live source-bound qPCS
verifier. The [private original-owner handoff](../docs/history/2026-09-24/mkhe-f06-original-source-budget-owner-handoff.md)
passes 2 focused checks without constructing live source correspondence;
full additive work admission and the cryptographic redesign remain open.
The [X509 proof-geometry audit](../docs/history/2026-09-23/zk-x509-proof-geometry-audit.md)
quantifies the complete maximum relation above the fixed 9 MiB ceiling;
the proof redesign and soundness evidence remain open.
The [BFV governed-replay validation](../docs/history/2026-09-23/bfv-governed-replay-native-verifier.md)
records the unselected-coefficient adversary and native artifact-aware verifier;
the production qualification and full relation evidence remain open.
The [AXT redacted-amount validation](../docs/history/2026-09-23/axt-redacted-amount-validation.md)
records a focused host/Core/state hard refusal while confidential proof and
durable nonce admission remain open.
The [AXT nonce State-owner audit](../docs/history/2026-09-23/axt-anchored-spend-nonce-state-owner.md)
pins the missing signed-spend carrier and atomic application/replay boundary;
the production hard refusal remains in force.
The [standalone election semantic guards](../docs/history/2026-09-23/election-semantic-guard-validation.md)
pass three focused negative controls while the anonymous dropout-resilient
protocol and its production circuits remain unresolved.
The [privacy reserve custody cutover](../docs/history/2026-09-23/privacy-public-reserve-custody-cutover.md)
records typed Orchard/private-IVM reserve custody installed atomically with
governed bootstrap, guards on ordinary transfers, burns, rekey and removal,
and exact verified-bridge routing with focused Core tests. Low-level apply-time
defense, charged lookup resources, full proof relations and conservation
qualification remain open.
The [C# host record](../docs/history/2026-09-21/csharp-native-runtime.md) records
native-backed tests and ordinary-stack admission without claiming release closure.
The [Android consumer record](../docs/history/2026-09-21/android-managed-host-runtime.md)
records managed and explicit host-JNI execution separately from device qualification.
The [deferred-handoff record](../docs/history/2026-09-21/deferred-handoff-carrier-validation.md)
records passing component controls and the remaining failed full liveness run.
The [18-step Apalache diagnostic](../docs/history/2026-09-23/multilane-apalache-vc-diagnostic.md)
identifies the timed-out verification conditions; it does not close the formal gate.
The [original-owner formal source binding](../docs/history/2026-09-24/multilane-original-owner-formal-source-binding.md)
passes the current structural checker and focused local-refusal mutations; it
does not close the pending formal or runtime qualification.
The [runtime VC mapping correction](../docs/history/2026-09-23/multilane-apalache-runtime-vc-mapping-correction.md)
locates the actual slow predicates, records two unsuccessful exact rewrites and
leaves the canonical model unchanged.
The [retained-source and qualification record](../docs/history/2026-09-21/retained-source-and-qualification-integration.md)
records first-mask ownership tests, fixture migration and published-schema work.
The [SCCP Java consumer record](../docs/history/2026-09-21/sccp-java-source-consumer-integration.md)
tracks assertion migration, the executed native evidence-validator join, and
the package producer with final candidate execution still outstanding.
The [prepared insertion and stream record](../docs/history/2026-09-21/prepared-insertion-and-s-stream-integration.md)
records bounded component ownership changes and their remaining qualification.
The [current checkout record](../docs/history/2026-09-21/current-optimizations-qualification.md)
separates newer runtime validation and executing Cell ownership from superseded
failures, and records [SDK input ownership](sorafs/reference_sdk_package_index.md)
and the [Python producer](sorafs/python_consumer_producer_v1.md) with its
[original-index verifier](sorafs/python_index_adapter_v1.md). The
[September 22 checkpoint](../docs/history/2026-09-22/python-index-integration.md)
records its source-observed component validation; it does not close F12 or SF11.
The [JavaScript archive checkpoint](../docs/history/2026-09-22/javascript-original-archive-integration.md)
adds bounded original npm/dependency ownership and records its component checks;
the installed JavaScript producer, original-index adapter and qualification remain open.
The [package/assertion checkpoint](../docs/history/2026-09-22/javascript-package-and-assertion-integration.md)
adds exact package/source projection, shared assertions and native cache
observation with 1,756 scoped controls; it does not qualify installed execution.
The [runtime-floor checkpoint](../docs/history/2026-09-22/javascript-node-floor-integration.md)
adds the shared Node minimum, privacy build ordering and repeatable local packing;
1,910 integrated controls pass while native and release qualification stay open.
The [Cargo/bundle checkpoint](../docs/history/2026-09-22/cargo-graph-and-javascript-bundle-integration.md)
reconciles the committed dependency pin and brings measured bundles within
unchanged ceilings; native, full source-seal and release qualification stay open.
The [test-event checkpoint](../docs/history/2026-09-22/javascript-test-event-integration.md)
adds the bounded fixed Node 24 case observer and CI wiring with 1,469 component
controls; actual installed source/runtime/native custody and execution remain required.
The [installed/snapshot checkpoint](../docs/history/2026-09-22/javascript-installed-and-snapshot-integration.md)
adds exact npm installed content, retained tree/native snapshot checks and the
shared directory-opener repair; 2,220 integrated controls pass while actual
installed execution, remaining publication cleanup and release authority stay open.
The [source/publication checkpoint](../docs/history/2026-09-22/javascript-source-and-publication-cleanup.md)
integrates the fixed qualification core and reviewed transaction cleanup with
2,069 affected controls. Same-process native execution, concurrent pathname
rollback, State admission/cutover and candidate release authority remain open.
The [fixed-child checkpoint](../docs/history/2026-09-22/javascript-fixed-child-integration.md)
integrates the same-session Node child and dependency-ordered CI controls;
1,764 component/static checks pass. Actual native execution and parent/index
authority remain open; the pending State cutover requires a reader-notification
fence correction before integration.
The [parent-input checkpoint](../docs/history/2026-09-22/javascript-parent-input-integration.md)
adds original input/file/tree custody and CI registration with 1,705 integrated
controls. Actual runtime, process, native and index authority remain open;
transitive State merge/autoscale read lifetimes still gate the runtime cutover.
The [runtime-input checkpoint](../docs/history/2026-09-23/javascript-runtime-input-integration.md)
adds bounded Node 24 Mach-O input graphs and CI registration with 1,735 scoped
passing controls. The actual 20-image runtime still rejects two shared `@rpath`
loads; loader, process, native and candidate authority remain open.
The [Node runtime canonical-producer record](../docs/history/2026-09-23/sorafs-node-runtime-canonical-producer.md)
derives those candidate slots from supplied original bytes and records the
20-image diagnostic pass; physical runtime use and independent release pins
remain open.
The [fixed child-process custody record](../docs/history/2026-09-23/sorafs-javascript-child-process.md)
adds bounded fd3/pipe/exit ownership and release-script registration; its 14
focused tests pass. Node image pinning, mapped-image verification, installed
execution, and candidate authority remain open.

The [history-admission checkpoint](../docs/history/2026-09-22/transaction-history-admission-integration.md)
integrates original Kura membership capacity, typed restore and retained
State/snapshot/replay read ownership. Core compilation and 897 configuration
tests pass; integrated runtime tests and complete Native cutover remain open.
The [atomic preparation slice](../docs/history/2026-09-23/transaction-history-atomic-preparation.md)
funds the prior-tip batch, next identity and actual history cursor together before
their construction; its focused Core suite passes 18/18, and F02 remains open.
The [hot-tip and snapshot admission audit](../docs/history/2026-09-23/transaction-hot-tip-snapshot-admission-gap.md)
identifies the charged-owner and local-retry cutover still required for those
allocations; no encoded-size proxy qualifies as full resource admission.
The [charged hot-tip source slice](../docs/history/2026-09-23/transaction-hot-tip-charged-cutover.md)
funds the retained shell and key backing and avoids a second set at State
staging. The fresh `storage_transactions::tests::` Core selector passes 22/22;
upstream carrier/merge allocations,
aggregate retry custody and snapshot scratch keep F02 open.
The [ordinary carrier source slice](../docs/history/2026-09-23/ordinary-carrier-membership-source-admission.md)
pre-funds signed ordinary replay-hash backing on the original State owner; the
post-merge `ordinary_signed_carrier_` Core selector passes 2/2. Producer, merge,
Native, alias scratch and aggregate admission still keep F02 open.
The [funded evidence-prune slice](../docs/history/2026-09-23/sumeragi-f02-funded-prune-path.md)
reserves fixed keys before a borrowed State scan and routes exact pool refusals
locally; a later borrowed admission scan removes the whole-table proof clone.
The latest combined Core evidence suite passes 38/38, the penalty selector
passes 24/24, and config integration passes 243/243. The
[borrowed penalty planner](../docs/history/2026-09-23/sumeragi-f02-borrowed-penalty-planning.md)
removes whole-proof and locator-list clones. Its
[single stake-share owner cut](../docs/history/2026-09-24/sumeragi-f02-single-stake-share-owner.md)
also removes the second full key inventory. Its focused test passes 1/1, and
the rebuilt penalty and evidence suites pass 24/24 and 38/38. The
[stake-index boundary](../docs/history/2026-09-23/sumeragi-f02-stake-index-original-owner-boundary.md)
records the still-unfunded shared State and local-retry owners. Full
proposal-snapshot, remaining admission and penalty funding from the
[design](../docs/history/2026-09-23/sumeragi-f02-evidence-preparation-admission-design.md)
remains open.
The [non-Copy charged-buffer foundation](../docs/history/2026-09-24/sumeragi-f02-noncopy-charged-buffer-foundation.md)
passes its focused MV target 30/30; nested stake-index and State custody remain open.
The [membership local-refusal classification](../docs/history/2026-09-23/transaction-membership-local-refusal-classification.md)
keeps ordinary and Native carrier staging capacity errors out of deterministic
block rejection; it does not fund upstream sources or snapshot scratch.
The [combined-source Core type repair](../docs/history/2026-09-23/combined-source-core-type-repair.md)
rejoins transaction append preparation identity and typed reserve restore tests
after the concurrent source merge; focused and full qualification remain open.
The [combined-source file-budget diagnostic](../docs/history/2026-09-23/combined-source-file-budget-gate.md)
records 235 current findings after exact downward ratchets and mechanical
test splits, including the
[Sumeragi release-bootstrap test split](../docs/history/2026-09-23/sumeragi-release-bootstrap-test-budget-split.md).
No budget baseline was raised; the release tooling gate remains open.
The [Sumeragi relay ingress fail-close](../docs/history/2026-09-23/sumeragi-relay-ingress-fail-closed.md)
returns unowned relay variants before enqueue while preserving QueuePlan
transfer. Network rejection is terminal and the finalized Nexus record may be
retried; the authoritative Native/Nexus owner and F03 cutover remain open.
The [September 23 integrated packet checkpoint](../docs/history/2026-09-23/integrated-packet-validation.md)
records scoped MV, MKHE, signer and Node input tests, plus the current Core
history regression and its fixture corrections. None closes a production or
release gate.
The [same-day election, signer, provider and JavaScript checkpoint](../docs/history/2026-09-23/election-signer-provider-js-checkpoint.md)
records later scoped source changes and the exact local test results, including
the still-rejected standalone ZK election, SoraFS grant, BFV production and
Node runtime paths. It is not a frozen candidate receipt.
The [SoraFS signer package-metadata correction](../docs/history/2026-09-23/sorafs-signer-package-metadata.md)
removes unsupported backend/qualification claims while retaining strict
signed-operation and source-seal gates.
The [final-promotion state-source audit](../docs/history/2026-09-23/final-promotion-operation-state-source-gap.md)
pins the native Reserve/Complete and Check authorities and the missing daemon
submission, observer, clock, floor and private-receipt joins. Software custody
remains fail-closed until a production source supplies them.
The [release-manifest native operation boundary](../docs/history/2026-09-23/sorafs-release-manifest-native-operation-gap.md)
adds a bounded internal role-13 Norito claim contract while keeping release
signing disabled pending native custody, finalized operation state and
purpose-aware daemon dispatch; software signing needs no HSM.
The [software signer and Current Check validation](../docs/history/2026-09-23/sorafs-software-signer-current-check-validation.md)
records focused 5/5 provider, 10/10 Current Check, 3/3 credential and 4/4
provider-assignment controls; those component passes do not authorize promotion.
The [topology native-authority prerequisite](../docs/history/2026-09-23/sorafs-topology-native-authority-prerequisite.md)
keeps promotion blocked after signed-envelope replay until a completed
role-16 operation is verifiable from finalized state.
The [signed-topology type-binding control](../docs/history/2026-09-23/sorafs-topology-aggregate-type-binding.md)
rejects numeric JSON substitutions between the signed topology and aggregate;
native role-16 authority and promotion remain blocked.
The [topology inner-approval claim boundary](../docs/history/2026-09-23/sorafs-topology-inner-approval-claim-boundary.md)
also checks the exact detached binding and independently pinned signer identity;
it cannot replace the missing role-16 receipt and finalized Check.
The [resilience inner-approval claim boundary](../docs/history/2026-09-23/sorafs-resilience-inner-approval-claim-boundary.md)
checks the exact signed summary projection and signer tuple; its separate
native purpose, completion and finalized Check are still absent.
The [lane-inventory inner-approval claim boundary](../docs/history/2026-09-23/sorafs-lane-inventory-inner-approval-claim-boundary.md)
checks exact replay bytes, signer role and topology anchors; a native
purpose-owned completion and finalized Check are still absent.
The [foundational inner-approval claim boundary](../docs/history/2026-09-23/sorafs-foundational-inner-approval-claim-boundary.md)
requires explicit software-receipt replay and exact candidate-prerequisite
bindings; finalized custody and native completion are still absent.
The [provider-ingest G06 authority audit](../docs/history/2026-09-23/sorafs-provider-ingest-g06-authority-audit.md)
separates finalized assignment lookup from the still-missing finalized
admission, advert, revocation and governed grant source.
The [read-only current source-assignment service](../docs/history/2026-09-23/sorafs-provider-ingest-g06-current-source-assignment-service.md)
exposes a committed-head-bound canonical request and recheck to a separate
resolver; it does not provide those missing governance and grant authorities.

The component ledgers remain the detailed acceptance authorities:
[privacy](privacy_first_release_closure.md),
[SoraFS](sorafs/v1_implementation_goals.md), and
[multilane](sumeragi_v2_multilane_completion_goals.md), including the
[liveness redesign](sumeragi_liveness_redesign_goals.md). A source change,
compiled profile, historical receipt, or synthetic fixture does not close a
release goal.

## Fixed requirements

- Work only in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`.
- First release means **no backward compatibility**: one final V1 implementation
  and wire contract; delete retired aliases, shims, fallback decoders, and
  competing legacy execution paths. Canonical account aliases remain ordinary
  product functionality, not compatibility aliases.
- IVM is the only VM. Use Norito; retain mandatory signed RS16 availability,
  exact validator quorums, deterministic execution, and committed policy.
- Privacy STARK outer commitments/transcripts use SHA3-384. Execution STARK,
  FASTPQ, BFV, and inner relations retain their specified six-lane constructions.
  This explicitly replaces the former blanket six-lane requirement.
- Preserve X509 certificate coverage and existing production proof, work,
  resident-memory, spool, and I/O ceilings. Oversized fixtures do not qualify.
- Authenticated software signing is the SoraFS release contract. HSM access is
  not a prerequisite. Authorization, revocation, rotation, durable completion,
  and independent evidence remain mandatory.
- Standalone elections hide credential identity and ballot choices beyond what
  final totals imply. Relayers do not become voter authority. Ordinary ballots
  are immutable; conviction updates preserve the choice and only increase the
  bond or extend its lock. Count only the latest accepted weight.
- Conviction weight uses exact smallest asset units with a frozen asset scale,
  the quadratic/conviction formula, and checked integer/aggregate bounds.
- Elections allow bounded voter phases and require late-dropout completion.
  No decryption committee, master decryption key, voter-secret reconstruction,
  omission of accepted ballots, or extra subset tally may supply recovery.
  A protocol meeting these requirements is still a cryptographic design
  deliverable; no rejected trustee or voter-held recovery design is approved.
  The [standalone protocol contract](standalone_election_protocol_contract.md)
  defines the required fault, phase, disclosure and rejection boundaries.

## Ordered goals

| ID | State | Outcome and owner | Completion criteria |
| --- | --- | --- | --- |
| F01 | Active | Source reconciliation — integration owner | Preserve SoraFS tracked/untracked source and original Git identity; reconcile onto current source without overwriting newer owners; integrate software-custody changes and regenerate combined-source artifacts. |
| F02 | Active | Resource admission and retained execution — Core/storage owners | Reserve actual allocations and side effects before work; retain original State/Queue, journals and resource owners from validation to publication, refusal and restart; execute each accepted subject once. The [borrowed indexed-exposure cut](../docs/history/2026-09-24/sumeragi-f02-borrowed-indexed-exposure.md) passes 1/1; [pending-penalty backing](../docs/history/2026-09-24/sumeragi-f02-pending-penalty-backing.md) passed Config 2/2 and Core 7/7; [nested peer-key charging](../docs/history/2026-09-24/sumeragi-f02-pending-peer-key-charge.md) passes Config 2/2 and Core 4/4. The [stake-index original-owner cut](../docs/history/2026-09-24/sumeragi-f02-stake-index-demand-design.md) removes its B-tree and funds two flat backings plus exact nested single/multisig AccountId clones from the original State pool; DataModel 1/1, Config 1/1 and corrected same-source Core 18/18 pass, including capacity refusal, partial-clone refund, exposure, retry and local DecodeScope classification. The pinned native-digit Quantity clone prerequisite passes vendor 1/1 and Primitives 2/2; borrowed validator-total comparisons pass same-source Core 10/10. The [borrowed stake-group preflight](../docs/history/2026-09-24/sumeragi-f02-borrowed-stake-group-preflight.md) passes Core demand 8/8, malformed-source 5/5, exposure 1/1 and capacity-retry 1/1, while F02 remains open. The [borrowed canonical ingress binding](../docs/history/2026-09-24/sumeragi-f02-canonical-ingress-borrowed-binding.md) removes one duplicate request clone and passes Core worker 3/3. The [streamed exact-byte comparison](../docs/history/2026-09-24/sumeragi-f02-streamed-exact-ingress-comparison.md) removes the second output buffer and passes Core exact-byte 2/2, owner 1/1 and worker 3/3; the [canonical recovery frame-length cut](../docs/history/2026-09-24/sumeragi-f02-canonical-recovery-frame-length.md) passes Core worker 1/1 and overflow 1/1, removing the outer frame-check clone and buffer; nested serialization scratch remains unfunded. The [physical-charge audit](../docs/history/2026-09-24/sumeragi-f02-canonical-ingress-physical-charge-audit.md) identifies the absent original ingress lease, pre-admission encoding allocation and next reservation cut; it ran no tests. State-pool charge integration, Quantity addition scratch, 32-bit and full-workspace checks, source lookup, per-member work admission, maximum-shape runtime bounds, scratch State, action vectors and aggregate admission remain open. |
| F03 | Active | Native lane cutover and lifecycle — consensus owner | Shared reducer, pre-payload timeout/replacement, exact frozen contexts, durable votes and RS16 availability; complete original Apply consumer and remove old signer together; evidence-aware drain/archive/recreation. The [runner/source audit and bounded canonical worker](../docs/history/2026-09-23/sumeragi-f03-native-runner-source-and-evidence-audit.md) separate connected production code from prior four-peer evidence; worker tests pass 2/2 and four adjacent recovery consumers pass 1/1 each, while live ingress/output remains closed. The [canonical recovery output allowlist](../docs/history/2026-09-24/sumeragi-f03-canonical-recovery-output-allowlist.md) passes its focused selector 1/1 and five adjacent selectors 5/5. The [silent initial-author Native driver test](../docs/history/2026-09-24/sumeragi-f03-silent-initial-author-native-driver-evidence.md) passes 1/1 with three surviving physical drivers, an exact timeout quorum, frozen signed RS16 replacement proposal and surviving Commit quorum; P2P outage/restart qualification remains open. The [finalized-closed Native stale-control test](../docs/history/2026-09-24/sumeragi-f03-finalized-closed-native-stale-control.md) passes Core 1/1 and preserves the original fair-ingress owner across closure before rejecting the old signed artifact; true archive/recreation and restart remain open. The [missing-execution-evidence restart audit](../docs/history/2026-09-24/sumeragi-f03-missing-native-execution-evidence-restart-audit.md) confirms the required four-peer identity is absent while production still rejects Native economic carriers; G-4P remains open. Full lifecycle qualification depends on F02 ownership and admission. |
| F04 | Active | Software signing and promotion authority — SoraFS/daemon owners | Production providers/state sources and all four operation purposes; exact role authorization, reservation/completion/recovery; independently verified topology, inventory, resilience and foundational inner approvals. The [role-13 software release-manifest service assembly](../docs/history/2026-09-24/sorafs-release-manifest-software-service-assembly.md) passes 2/2 focused real-key/preflight tests through the existing durable operation service; configured finalized authority remains absent. The [role-13 finalized-source audit](../docs/history/2026-09-24/sorafs-release-manifest-role13-finalized-source-audit.md) confirms Core actions still reject, the raw custody reader lacks operation/finality evidence, and daemon sources are test-only. The [role-13 current block-finality readback](../docs/history/2026-09-24/sorafs-role13-custody-block-finality-readback.md) and [role-11 stream-token current readback](../docs/history/2026-09-24/sorafs-stream-token-role11-current-finality-read.md) pass fresh merged Core 1/1 each with software-custody fixtures; neither establishes executed Check, completed operation, production state-source or signing authority. The [role-15 one-use software key](../docs/history/2026-09-24/sorafs-final-promotion-role15-software-key.md) passes 4/4 focused and 24/24 adjacent daemon account tests. The [operation-source cut](../docs/history/2026-09-24/sorafs-final-promotion-operation-source-audit.md) binds Reserve/Complete rows to direct signed-entry origins and verifies the original signed Reserve against the same finalized Check cut; DataModel tests pass 25/25, Core same-block 1/1, current finalized observation 41/41 including missing-membership/finality rejection, and native operation 47/47. The daemon Current/Reserved source and exact-height pre-submit State/Kura/QC floor check pass 16/16 focused tests, including wrong-context refusal. The [two-floor recovery design](../docs/history/2026-09-24/sorafs-final-promotion-two-floor-recovery-design.md) now has a private pending-Reserve journal with an ID-specific durable in-progress tombstone and atomic no-replace publication; final-source journal 13/13 and pending-Reserve semantic 5/5 pass, alongside earlier runtime 18/18 and receipt-regression 9/9 evidence. An independent floor issuer and pin, full successor provenance, historical two-floor proof, ambiguous restart reconciliation, funded replay, Complete source, production providers, and [interface regeneration](../docs/history/2026-09-24/sorafs-final-promotion-f04-interface-regeneration-inventory.md) remain open. The [topology typed closed-admission cut](../docs/history/2026-09-24/sorafs-topology-typed-closed-admission.md) passes DataModel 2/2, schema 1/1, Core 1/1 and reducer 24/24; it does not supply role-16 native authority. The [topology source audit](../docs/history/2026-09-24/sorafs-topology-inner-approval-native-source-audit.md) records seven negative cases; promotion remains blocked. |
| F05 | Active; production grant blocked | SoraFS service backends — service owners | Finalized-ledger ingest/reputation, quarantine, PoP, moderation/viewer/appeal, PoTR, DAG and transparency; concrete supervised backends, bounded outboxes and crash/rotation recovery. The [provider-ingest sealed restart replay](../docs/history/2026-09-24/sorafs-provider-ingest-sealed-restart-owner-replay.md) passes 1/1 focused outbox test, and the [retained admission-cursor ancestry check](../docs/history/2026-09-24/sorafs-provider-ingest-retained-admission-ancestry.md) passes its full 12/12 daemon module after the fixture correction. The finalized assignment-to-outbox source is connected; the [native council producer design](../docs/history/2026-09-24/sorafs-provider-ingest-native-council-producer-design.md) fixes the next V1 authority cut. The [council-signed admission lineage wire](../docs/history/2026-09-24/sorafs-council-signed-admission-lineage-wire.md) now binds policy claims, event revisions, and exact predecessors with no retired decoder. The [canonical governed-policy candidate](../docs/history/2026-09-24/sorafs-f05-governed-council-policy-candidate.md) passes DataModel 3/3 for policy lineage, signatures, pause and expiry; it is not enacted and its 32-key validation follows generic decode allocation. The [native enactment audit](../docs/history/2026-09-24/sorafs-f05-native-council-enactment-audit.md) identifies the singleton Parliament/World cut, pre-allocation signer bound, and required closure of local-config admission authority; it ran no tests. Native pre-allocation admission, governed policy/head/tombstone, exact finalized grant reader, service backends, and distributed qualification remain open. |
| F06 | Active | Native40/MKHE/Vega — proof owners | Authenticated source replay, sole commitment/opening inventory, complete delta producers, consuming composite admission, governed full-shape keys and qPCS redesign inside unchanged whole-proof limits. The [canonical40 profile boundary](../docs/history/2026-09-24/mkhe-native40-governed-profile-source-boundary.md) passes focused profile 14/14, publisher 19/19, manifest 8/8 and source-preflight 1/1 checks; live coefficient correspondence, 38-limb consumer cutover and audited evidence remain open. The [qPCS source-bound redesign prerequisite](../docs/history/2026-09-24/mkhe-f06-qpcs-source-bound-redesign-prerequisite.md) records that each current initial-tree component exceeds the whole-proof tracked-work ceiling and identifies the required source/opening relation. The [original-budget owner handoff](../docs/history/2026-09-24/mkhe-f06-original-source-budget-owner-handoff.md) passes 2/2 focused checks, keeps the single opening inventory and initial-opening charge on the original ledger, and has no live source-correspondence constructor; full qPCS accounting and production composite admission stay closed. |
| F07 | Open | FASTPQ/AXT/X509 — proof and Core owners | Bounded compact proof admission, complete successful-execution and authoritative-state binding, durable spend nonces, and full X509 coverage within existing ceilings. The [fixed-row budget audit](../docs/history/2026-09-23/fastpq-f07-fixed-row-opening-budget-audit.md) and [source-derived size screen](../docs/history/2026-09-24/fastpq-source-budget-and-deep-admission-boundary.md) record the hard opening gap. The [checked source projection](../docs/history/2026-09-24/fastpq-f07-source-projection-and-prover-blocker.md) passes 1/1 focused test; the [original DEEP preflight](../docs/history/2026-09-24/fastpq-f07-deep-preflight-and-resource-floor.md) passed 6/6 under the former `<N` geometry. The [doubled-degree terminal boundary](../docs/history/2026-09-24/fastpq-f07-doubled-degree-terminal-boundary.md) passes 53/53 test-only DEEP selections, binds `<2N` transcript geometry and still refuses all private proving. The [base-field hiding and degree screen](../docs/history/2026-09-24/fastpq-f07-basefield-hiding-degree-screen.md) retains an unimplemented 508,399-byte candidate with soundness, privacy, resource and AXT-fit gates open. The [fixed-arity DEEP FRI fiber](../docs/history/2026-09-24/fastpq-f07-fixed-fri-fibers.md) passes 55/55 focused tests and measures 500,783 bytes for the inactive maximal candidate frame; the recorded production proof remains roughly 8 MB, with masked proving, soundness and Core admission open. The [AXT nonce cutover record](../docs/history/2026-09-24/axt-f07-durable-spend-nonce-cutover-blocker.md) identifies the missing authoritative signed-spend carrier. The [all-P-256 X509 bound](../docs/history/2026-09-24/zk-x509-all-p256-opening-bound.md) proves that removing those openings alone still exceeds 9 MiB; the [P-256/SHA recursive size contract](../docs/history/2026-09-24/zk-x509-p256-sha-recursive-size-contract.md) is an unimplemented full-coverage candidate with a 737,430-byte receipt allowance under fixed-cost accounting. The [complete MAIN registration partition audit](../docs/history/2026-09-24/zk-x509-complete-registration-partition-audit.md) passes Core 2/2 and accounts for all 49 registrations/5,623 columns, including 190 retained scalar-bit-bus columns; it changes no proof or gate. |
| F08 | Open | BFV qualification — crypto and independent reviewers | Full BFV-RNS relation, full-size/eight-party adversaries and measured bounds; real governed parameter/lattice/noise/qROM evidence before production qualification can accept. The [registered eighth-limb source adversary](../docs/history/2026-09-24/bfv-registered-operand-eighth-limb-mismatch.md) passes 1/1 focused test; it is neither the full relation nor an eight-party transcript. The [eight-party source audit](../docs/history/2026-09-24/bfv-eight-party-verification-prerequisite.md) records the pre-cut absence of participant interfaces. The [fail-closed decryption boundary](../docs/history/2026-09-24/bfv-eight-party-fail-closed-boundary.md) passes 5/5 focused tests with eight independently seeded parties, fixed roster and bounded Norito decode; authentication does not verify shares, and its public result path always rejects. The [eight-limb operand-to-source-product replay](../docs/history/2026-09-24/bfv-eight-limb-operand-source-replay.md) passes the focused crypto selector 1/1 and complete registered source-bound module 3/3, including an in-bound cancelling forgery; it validates arithmetic witnesses without admitting proofs. The private/masked contribution design, full RNS relation, governed signer profile and matching decoder ceilings, durable authority/replay, audits and production measurements remain open. Strict Clippy remains blocked by existing lints in unrelated dependency and crypto paths; a scoped run allowing those five lint classes passes. The separate 40-limb replay belongs to F06's MKHE composite. |
| F09 | Open | Exact12 completion — engine owners | Complete engine-specific soundness/key/provenance work and every adversarial, maximum-shape, resource, native/SDK/hardware and deployment requirement in the privacy ledger. The [Pasta SHA output-binding audit](../docs/history/2026-09-24/f09-pasta-sha-terminal-output-binding-audit.md) finds no confirmed missing terminal constraint in the scoped production path, distinguishes an unused claim view from the constrained job, and leaves an integrated adversarial tamper test open. |
| F10 | Open | Product privacy integration — product/Core owners | Confidential authority/conservation; Kaigi proof/relay/lifecycle; Parliament ballot/deadline/beacon/restart and independent protocol review. The [relay lifecycle guard](../docs/history/2026-09-24/kaigi-relay-route-lifecycle-guard.md), [restore key check](../docs/history/2026-09-24/kaigi-active-relay-restore-key-binding.md), and [feedback source/lifecycle cut](../docs/history/2026-09-24/kaigi-relay-feedback-source-lifecycle.md) pass 13/13 combined focused Core selectors. The [historical reporter source audit](../docs/history/2026-09-24/kaigi-f10-historical-reporter-source-audit.md) shows restore does not authenticate `reported_by` and the rekey record lacks transition height; signed-source and historical host-state replay are needed. The [Parliament private-ballot restart audit](../docs/history/2026-09-24/parliament-f10-private-ballot-restart-source-audit.md) distinguishes existing reducer/finality/restore guards from the missing later-phase four-validator and deployment-custody qualification. Historical reporter authorization across rekeys, relay deployment, multi-validator restart, and independent review remain open. |
| F11 | Active; private protocol unresolved | Standalone elections — protocol and product owners | Reviewed construction satisfying every fixed election requirement, then dedicated credential/ballot/tally circuits, confidential bond positions, exact closed-corpus state, SDKs and dropout/restart/resource qualification. The [late-dropout aggregate-opening review](../docs/history/2026-09-23/standalone-election-dropout-aggregate-blocker.md), [fault matrix](../docs/history/2026-09-24/standalone-election-dropout-fault-matrix.md), [primary-source functional-opening review](../docs/history/2026-09-24/standalone-election-primary-source-functional-opening-review.md), [NARAD subset-opening review](../docs/history/2026-09-24/standalone-election-narad-subset-opening-review.md), and [final-corpus opening audit](../docs/history/2026-09-24/standalone-election-final-corpus-opening-audit.md) record the scoped completion and leakage gaps without claiming impossibility. The [ODSUM fixed-zero-slot candidate](../docs/history/2026-09-24/standalone-election-odsum-zero-slot-rejection.md) is rejected because public ciphertext ratios reveal the hidden choice and a roster-to-zero-slot dropout blocks completion; the private-election gate remains closed. [Smallest-unit arithmetic](../docs/history/2026-09-24/standalone-conviction-smallest-unit-boundary.md) passes focused public tests. The [exact u128 tally representation](../docs/history/2026-09-24/standalone-election-u128-tally-representation.md) passes DataModel 1/1, Core 8/8 including atomic restore, Torii response 3/3 and HTTP selector 4/4, IVM ABI 11/11 and mock 1/1, isolated JavaScript builder/reader tests 16/16, Kotlin/Java-source consumer tests 5/5, Python source with an existing native wheel 135/135, and C# 5,788/5,788. Swift passes actual-source parser typecheck and scanner smoke; full Swift tests await a same-source native bridge. Full-host IVM, same-source native and complete SDK parity remain open. The [zero-minimum bond custody cut](../docs/history/2026-09-24/standalone-election-zero-minimum-bond-escrow.md) passes Core helper 1/1, adjacent PLAIN 4/4, grouped conviction integration 5/5 and guarded ZK lock rejection 1/1: even a zero minimum now requires escrow for a positive bond, with exact delta movement. The [immutable public cast and typed conviction update](../docs/history/2026-09-24/standalone-election-immutable-plain-cast-update.md) passes same-source Core library selectors, DataModel codec 1/1, grouped Core integration 9/9, and isolated JavaScript checks 7/7; same-source native and full SDK parity remain open. Public conviction arithmetic and real bond conservation are prerequisites, not anonymous-election completion. |
| F12 | Active; final regeneration after interfaces settle | Canonical APIs and SDK/native packages — SDK/release owners | One typed V1 surface; double regeneration; matching Rust, Kotlin/Java consumers, Swift, JS, Python and C# execution; complete five-target native artifacts and installation checks. The [current Swift attempt](../docs/history/2026-09-23/swift-current-checkout-build-prerequisite.md) stopped at package resolution because the same-source ABI-23 bridge has not yet been built. The [public-ballot update SDK slice](../docs/history/2026-09-24/standalone-election-immutable-plain-cast-update.md) passes isolated JavaScript boundary/declaration checks and compiles Kotlin and Java-source consumers; Kotlin runtime tests require a same-source ABI-23 native bridge, and full SDK parity remains open. The [optional-orderbook lazy cut](../docs/history/2026-09-24/javascript-f12-orderbook-lazy-bundle-boundary.md) passes the complete JavaScript bundle gate under the unchanged production ceilings. The [C# direct conviction update](../docs/history/2026-09-24/csharp-f12-plain-conviction-direct-v1.md) passes a warning-free Release build, 4/4 focused tests, and 5,796/5,796 unfiltered Release tests after repairing the exact KAGEMUSHA credential field. The [Python choice-free update](../docs/history/2026-09-24/python-f12-choice-free-conviction-instruction.md) passes native Rust 1/1 and scoped syntax/lint, but the same-source ABI3 wheel fails macOS dyld `mis-aligned LINKEDIT` before installed-package pytest collection; Python runtime parity remains open. The [canonical conviction fixture inventory](../docs/history/2026-09-24/f12-conviction-fixture-contract-inventory.md) identifies the Rust-authored direct-frame golden and Ordinary/QueuePlanSynced signed-builder mismatch; its repo-local fixture staging passed focused exporter/xtask tests 1/1 each and actual 27-entry, 32-file byte-identical publication. Cross-SDK golden parity and native-backed package qualification remain open. |
| F13 | Active; final runs after implementation | Formal/runtime/hardware qualification — validation owners | Required formal bounds, all current-Native four-validator scenarios, larger-network resilience/scaling, full-proof hardware parity/fault quarantine and unchanged-candidate resource evidence. The [current script-suite collection boundary](../docs/history/2026-09-24/multilane-script-suite-collection-boundary.md) and [retired-validator assertion map](../docs/history/2026-09-24/multilane-retired-validator-assertion-map.md) record the V1 migration. The [original-owner structural binding](../docs/history/2026-09-24/multilane-original-owner-formal-source-binding.md) passes the canonical checker and 9 focused cases, including 8 mutations, without a completed formal engine run. The canonical descriptor runner, archive schema, control binding and journal owners pass scoped Python suites (31, 35, 143, 326, 527, 333 cases respectively); native-facts and completed-owner modules pass together 135/135, including duplicate-account and equal-valued lag type refusal; runner-result selection passes 168 with one pre-existing stale source-digest assertion deselected. The remaining native proof/receipt consumer and source-pinned candidate inventory prevent release qualification. |
| F14 | Queued after F01–F13 | Audit, candidate sealing and promotion — release/operators/reviewers | Resolve required audit findings; exact-source complete validation; genuine signed release and promotion evidence, authenticated publication/readback, and rollback qualification. The [interim source-file budget diagnostic](../docs/history/2026-09-24/source-file-budget-interim-diagnostic.md) reports 239 current-source violations; it is not a frozen-candidate receipt. |

Work on independent owners proceeds concurrently. Shared Core/State/Queue/Kura
changes are reconciled by one integration owner. Do not turn on Native ingress
or a proof engine before its complete authoritative production consumer exists.

## Candidate acceptance

- Privacy: all twelve protocols, genuine 48-stage/54-artifact evidence,
  independent review, native/SDK/hardware qualification, and four-validator
  lifecycle, restart, convergence and endpoint readback.
- SoraFS: four voting validators, multiple providers, two regional gateways,
  two DAG instances, 1,000 concurrent streams, corruption/load/recovery testing,
  a 24-hour soak, disaster recovery and all 17 fresh signed readiness summaries.
- Multilane: all six milestones and seven release gates, including the final
  18-step Apalache obligation, 13 global validators and three four-validator
  dataspaces, ten deterministic seeds and a two-hour fault soak.
- Scaling: five fixed-workload one-/four-lane pairs, at least 1.5x throughput
  and at most 1.25x p95 latency, with complete resource maxima through drain.
- Release: locked workspace builds/tests, strict Clippy, formatting, codec
  guards, script suites, double fixture/OpenAPI regeneration, five-target native
  reproducibility/install smokes, provenance, SBOMs and dependency scans.
- One immutable source/lock/toolchain/configuration/artifact identity must join
  every required final receipt. Failed, timed-out, skipped, unavailable and
  historical runs are never passes for that candidate.

## Progress and evidence discipline

Update the corresponding goal and component ledger when an outcome is actually
implemented and verified. Record exact commands, source scope, failures and
remaining limitations in dated implementation records. Keep `status.md` and
`roadmap.md` within their 300-line limits.

Independent audits, trusted operator signatures, runtime credentials and
physical test runners are external inputs. Their absence does not stop
independent source implementation, and synthetic replacements cannot close
their gates. If the required election construction is not established, retain
F11 and final release as open while completing the other workstreams.
