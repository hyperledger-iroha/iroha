# First-release completion goals

Set: 2026-09-20. Overall goal: **Active**. This record coordinates the accepted
privacy/ZK, SoraFS V1, and multilane implementation and qualification plan.
Starting checkout: `f11eed2d6c7113163295e65703d9c206a5ce07d9` (`optimizations`).
Integration branch: `codex/complete-privacy-sorafs-multilane`.
The [integration checkpoint](../docs/history/2026-09-20/first-release-integration.md)
records preservation, implementation boundaries and scoped validation failures.
The [September 21 runtime checkpoint](../docs/history/2026-09-21/runtime-and-governance-integration.md)
and [SDK response checkpoint](../docs/history/2026-09-21/js-governance-response-validation.md)
record subsequent integrated checks. The [native40 migration record](../docs/history/2026-09-21/mkhe-native40-migration.md)
separates completed evidence-identity hardening from unresolved production work.
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

The component ledgers remain the detailed acceptance authorities:
[privacy](privacy_first_release_closure.md),
[SoraFS](sorafs/v1_implementation_goals.md), and
[multilane](sumeragi_v2_multilane_completion_goals.md), including the
[liveness redesign](sumeragi_liveness_redesign_goals.md). A source change,
compiled profile, historical receipt, or synthetic fixture does not close a
release goal.

## Fixed requirements

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
| F08 | Open | BFV qualification — crypto and independent reviewers | Full relation/40-limb replay, full-size/eight-party adversaries and measured bounds; real governed parameter/lattice/noise/qROM evidence before production qualification can accept. |
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
