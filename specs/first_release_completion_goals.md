# First-release completion goals

Set: 2026-09-20. Overall goal: **Active**. This record coordinates the accepted
privacy/ZK, SoraFS V1, and multilane implementation and qualification plan.
Starting checkout: `f11eed2d6c7113163295e65703d9c206a5ce07d9` (`optimizations`).
Required working directory: `/Users/takemiyamakoto/devstuff/iroha`.
Required branch: `optimizations`. Perform all further implementation and
validation here; do not create or use another checkout, worktree or branch.
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
The [X509 proof-geometry audit](../docs/history/2026-09-23/zk-x509-proof-geometry-audit.md)
quantifies the complete maximum relation above the fixed 9 MiB ceiling;
the proof redesign and soundness evidence remain open.
The [BFV governed-replay validation](../docs/history/2026-09-23/bfv-governed-replay-native-verifier.md)
records the unselected-coefficient adversary and native artifact-aware verifier;
the production qualification and full relation evidence remain open.
The [AXT redacted-amount validation](../docs/history/2026-09-23/axt-redacted-amount-validation.md)
records a focused host/Core/state hard refusal while confidential proof and
durable nonce admission remain open.
The [standalone election semantic guards](../docs/history/2026-09-23/election-semantic-guard-validation.md)
pass three focused negative controls while the anonymous dropout-resilient
protocol and its production circuits remain unresolved.
The [C# host record](../docs/history/2026-09-21/csharp-native-runtime.md) records
native-backed tests and ordinary-stack admission without claiming release closure.
The [Android consumer record](../docs/history/2026-09-21/android-managed-host-runtime.md)
records managed and explicit host-JNI execution separately from device qualification.
The [deferred-handoff record](../docs/history/2026-09-21/deferred-handoff-carrier-validation.md)
records passing component controls and the remaining failed full liveness run.
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
The [software signer and Current Check validation](../docs/history/2026-09-23/sorafs-software-signer-current-check-validation.md)
records focused 5/5 provider, 10/10 Current Check, 3/3 credential and 4/4
provider-assignment controls; those component passes do not authorize promotion.
The [topology native-authority prerequisite](../docs/history/2026-09-23/sorafs-topology-native-authority-prerequisite.md)
keeps promotion blocked after signed-envelope replay until a completed
role-16 operation is verifiable from finalized state.
The [provider-ingest G06 authority audit](../docs/history/2026-09-23/sorafs-provider-ingest-g06-authority-audit.md)
separates finalized assignment lookup from the still-missing finalized
admission, advert, revocation and governed grant source.

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
| F02 | Active | Resource admission and retained execution — Core/storage owners | Reserve actual allocations and side effects before work; retain original State/Queue, journals and resource owners from validation to publication, refusal and restart; execute each accepted subject once. |
| F03 | Queued after F02 | Native lane cutover and lifecycle — consensus owner | Shared reducer, pre-payload timeout/replacement, exact frozen contexts, durable votes and RS16 availability; complete original Apply consumer and remove old signer together; evidence-aware drain/archive/recreation. |
| F04 | Active | Software signing and promotion authority — SoraFS/daemon owners | Production providers/state sources and all four operation purposes; exact role authorization, reservation/completion/recovery; independently verified topology, inventory, resilience and foundational inner approvals. |
| F05 | Queued after F01 | SoraFS service backends — service owners | Finalized-ledger ingest/reputation, quarantine, PoP, moderation/viewer/appeal, PoTR, DAG and transparency; concrete supervised backends, bounded outboxes and crash/rotation recovery. |
| F06 | Active | Native40/MKHE/Vega — proof owners | Authenticated source replay, sole commitment/opening inventory, complete delta producers, consuming composite admission, governed full-shape keys and qPCS redesign inside unchanged whole-proof limits. |
| F07 | Open | FASTPQ/AXT/X509 — proof and Core owners | Bounded compact proof admission, complete successful-execution and authoritative-state binding, durable spend nonces, and full X509 coverage within existing ceilings. |
| F08 | Open | BFV qualification — crypto and independent reviewers | Full BFV-RNS relation, full-size/eight-party adversaries and measured bounds; real governed parameter/lattice/noise/qROM evidence before production qualification can accept. The separate 40-limb replay belongs to F06's MKHE composite. |
| F09 | Open | Exact12 completion — engine owners | Complete engine-specific soundness/key/provenance work and every adversarial, maximum-shape, resource, native/SDK/hardware and deployment requirement in the privacy ledger. |
| F10 | Open | Product privacy integration — product/Core owners | Confidential authority/conservation; Kaigi proof/relay/lifecycle; Parliament ballot/deadline/beacon/restart and independent protocol review. |
| F11 | Active; private protocol unresolved | Standalone elections — protocol and product owners | Reviewed construction satisfying every fixed election requirement, then dedicated credential/ballot/tally circuits, confidential bond positions, exact closed-corpus state, SDKs and dropout/restart/resource qualification. Public conviction arithmetic and real bond conservation are prerequisites, not anonymous-election completion. |
| F12 | Active; final regeneration after interfaces settle | Canonical APIs and SDK/native packages — SDK/release owners | One typed V1 surface; double regeneration; matching Rust, Kotlin/Java consumers, Swift, JS, Python and C# execution; complete five-target native artifacts and installation checks. |
| F13 | Active; final runs after implementation | Formal/runtime/hardware qualification — validation owners | Required formal bounds, all current-Native four-validator scenarios, larger-network resilience/scaling, full-proof hardware parity/fault quarantine and unchanged-candidate resource evidence. |
| F14 | Queued after F01–F13 | Audit, candidate sealing and promotion — release/operators/reviewers | Resolve required audit findings; exact-source complete validation; genuine signed release and promotion evidence, authenticated publication/readback, and rollback qualification. |

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
