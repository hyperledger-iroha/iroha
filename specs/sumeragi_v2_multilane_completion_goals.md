# Production multilane implementation goals

Set: 2026-09-06. Overall goal: **Active**. No milestone is complete.

Implement and qualify the Production Multilane Completion Plan as Iroha 3's
first release. The [closure ledger](sumeragi_v2_multilane_closure_ledger.md)
owns detailed invariants, production symbols, adversarial cases and release
gates. This record owns the current execution order and audit corrections.

## Design decisions

- One canonical implementation and explicit versioned Norito layout per
  capability. Reject obsolete layouts; no backwards compatibility, aliases,
  mixed consensus operation, implicit decoding or migration shims.
- Native participant controls certify routing and settlement. Economic effects
  enter WSV exactly once through the canonical global carrier.
- Reuse and validate the implemented participant predicate, durable claims,
  manifests and autonomous execution paths. Replace a design only for a
  demonstrated correctness or ownership gap.
- Kotlin `core-jvm` owns JVM models and protocol validation. Migrate every Java
  assertion, fixture and delivery consumer before deleting duplicate Java
  implementations; Java-source tests call Kotlin directly. Preserve JDK 8 API
  enforcement, defensive ownership and Android module separation.
- Keep formal models in the existing `formal/sumeragi_v2` owner. Public operator
  guidance belongs in optional sibling `iroha-docs`; it is not a build dependency.
- Use twelve lane-validator assignments across three independent four-validator
  dataspaces within an exact thirteen-validator global committee. `G-12P` keeps
  its existing identifier; twelve global voters cannot form a `3f + 1` committee.
- No new crates, manifest/lock changes, direct Serde dependencies, ABI version
  changes, production feature/environment switches or hardware-dependent results.
  Runtime bounds belong in `iroha_config` with deterministic defaults.

## Ordered milestones

| Goal | State / dependency | Owner | Completion criteria |
| --- | --- | --- | --- |
| M1 — Reconcile baseline and invariants | **Active** | Core/Sumeragi, model, formal and release tooling | Inventory current TODOs across source and root records; map each to implementation, negative tests and formal evidence or a source-proven exclusion. Reconcile stale ledger assertions, generated-source inventories and current runner paths. Audit the shared participant-role predicate across validation, Kura, State, recovery, diagnostics and retirement. |
| M2 — Native security and durable application | Open; M1 | Native AMX, model, Kura and State | Revalidate and repair `ML-NAT-01`–`ML-NAT-07` and `ML-KURA-01`: full signed source/session/route claims; exact incarnation and successor; atomic bounded groups; QC-bound manifest/proof; finality → sidecar/index → frontier publication; bounded startup repair, pruning, archive and index recovery. |
| M3 — Autonomous execution and ownership | Open; M2 | Queue, lane consensus, merge and canonical application | Close `ML-QUEUE-01` and `ML-AUT-01`–`ML-AUT-06`: signed strict admission, FIFO fsynced exact reservations, validator-independent checks, durable certified bundles, contiguous canonical merge sources, exact-base re-execution and post-commit Commit/ForgetCommit. Recover or release every ownership crash boundary without loss or duplication. |
| M4 — Automatic lifecycle and convergence | Open; M3 | Autoscale, lifecycle, Kura and diagnostics | Close `ML-LIFE-01`–`ML-LIFE-05`: atomic create/activate, every queue/reservation/certified/delayed/merge/Native blocker, one evidence-aware drain frontier, archive-before-removal, new-incarnation recreation and delayed-artifact rejection. Complete relevant owner/worker handoffs and qualify the recovered-payload application path. |
| M5 — Canonical clients and wire corpus | Open; audit may run alongside M2–M4, final parity follows settled wire | Rust/OpenAPI, Python, JavaScript, Swift and Kotlin/JVM owners | Close `ML-API-01`–`ML-API-04` and `ML-WIRE-01`: separate status/diagnostics, bounded State/Kura-derived conflict-aware rows, identical strict Native V2 accept sets and Rust-owned grouped positive/negative fixtures. Migrate JVM production/tests/runners to Kotlin ownership and rederive source/artifact inventories. |
| M6 — Formal and release qualification | Open; M1–M5 | Formal, integration, performance and release owners | Close `G-UNIT`, `G-FORMAL`, `G-4P`, `G-12P`, `G-SCALE`, `G-SDK` and `G-FINAL` on one source-bound candidate. Archive exact commands, outcomes, hashes, tools, configuration, seeds and hardware. Update current status and public docs only from fresh passing evidence. |

Every milestone stays open until its focused negative tests and matching formal
mutations pass. Interface properties use their explicitly defined differential
or static checks, not invented TLA+ proofs. Source presence, test collection,
old logs, skipped tests and mutable-tree inventories cannot close a release gate.

## First work queue from the current audit

1. Classify and resolve the owner/worker completion, recovered Broadcast rollover,
   composite planner queue-cut and live Ingress handoff TODOs recorded in the
   ledger. Include the post-WAL-append crash-boundary TODO in
   `v2_ready_durable_validate_adapter_preview.rs`. Do not remove a TODO merely
   because a neighboring path exists.
2. Add direct classifier/helper negatives for Prepare/Commit identity drift,
   predecessor mismatch, route/dataspace/incarnation/view/proposal drift,
   settlement tampering and a malformed later leg after an earlier route match.
   Compare the Core role classifier with model grouped-source alignment without
   weakening either validation boundary.
3. Requalify signed QueuePlan intent against stripped gossip, external ordinary
   execution, follower validation and historical replay. The old queue TODO was
   stale: signed intent and rejection paths already exist. Its replacement is
   a comment correction, not a new security implementation or test pass.
4. Migrate Sumeragi/Native Java consumer suites into Kotlin-owned testing and
   remove duplicate implementation only after all assertions and delivery paths
   move. Replace mutable public diagnostic model collections with owned values.
5. Repair Linux formal/chaos workflow invocation paths: the Ubuntu workflow
   currently requests `mktemp` under macOS-specific `/private/tmp`. Preserve
   private directory permissions and invocation/artifact isolation.
6. Rebuild fixture and recursive SDK source inventories from their canonical
   producers. Current fixture hashes and suite counts differ from ledger pins;
   copying new hashes into pins alone cannot establish correctness.
7. Reconcile the existing unmerged index through its owning work before sealing
   a candidate. Preserve unrelated work; no release receipt can use this index.

## Required acceptance evidence

- Adversarial units cover drift/equivocation, stale incarnation/ABA, predecessor
  jumps, partial/duplicate/4,097-source groups, QC/manifest/proof/result forgery,
  eviction, malformed/oversized/symlinked storage, reservation duplication,
  base-state mismatch, bounded recovery and every persistence crash boundary.
- Current source-bound autoscale, Native application and autonomous carrier
  models pass TLC and Apalache; each mutation yields its expected counterexample.
  Retain the ledger's required deductive/production-trace obligations and hashes.
- Four-peer signed-RS16 suites cover automatic expansion/contraction, recovery,
  A/B/A same-ID recreation, grouped/mixed-role and same-route Native application,
  offline/Byzantine rotation, pruning, drain/archive and autonomous execution.
  Prove authenticated hold/drop acknowledgement before healing; skips fail.
- `G-12P`: 10/10 fresh deterministic seeds and a two-hour fault soak, grouped DvP
  plus autonomous work and lifecycle changes, full convergence, durable receipts,
  zero loss, zero rejected-after-acceptance transactions and zero duplicates.
- `G-SCALE`: pinned hardware, five paired one-lane/four-lane runs, at least 1.5×
  median committed throughput and at most 1.25× p95 latency at matched load.
  All queue, index, memory and disk bounds must hold.
- Before each Cargo invocation inspect `ps -axo pid,etime,command` and wait if
  Cargo/rustc is active. Never signal those processes. Use an isolated target,
  `--locked --offline` for build/test/lint, and a 20-minute build budget without
  treating timeout as success. Run focused crate/SDK suites and formal runners,
  then workspace build/tests, strict workspace all-target Clippy, `cargo fmt
  --all -- --check`, and the legacy-codec guard. Formatting does not resolve
  dependencies and does not accept Cargo build's lock/offline flags.
- No automatic release, deployment, signing bypass or fabricated pinned-hardware
  evidence. Keep unavailable external gates open and record their prerequisites.

## 2026-09-06 setup evidence

- Read the supplied plan, root status/roadmap and applicable repository rules;
  inspected Native, autonomous, lifecycle, client and formal source in parallel.
- Confirmed the old autonomous `execution_batch.is_none()` exclusion is absent
  and the canonical Native role predicate and V4 source claims already exist.
- `bash ci/check_sumeragi_v2_multilane_release_inventory.sh` exited 1: the SDK
  source-closure check rejected an existing unmerged Git index path. This is a
  current failed preflight, not a release or runtime result.
- Scoped `git diff --check`, local Markdown links, all six milestone entries
  and both root 300-line limits pass. Historical archive verification passes:
  64,736 records and 67,311 occurrences.
- `python3 scripts/formal/check_sumeragi_v2_multilane_models.py` exited 1.
  Reviewed Rust include inventories differ from current Kura/lane sources and
  State lacks a single stage-zero index entry. This is a structural failure;
  no TLC/Apalache execution or proof is implied.

## 2026-09-06 implementation checkpoint

- Kotlin's four remaining public diagnostics data classes are now immutable
  regular classes with explicit value equality. Private JSON serializers keep
  the exact wire fields/defaults; construction owns every list and nested JSON
  map/array, validates unsigned counters and rejects null list entries. Nesting
  and vector bounds apply before recursive copying. Existing copy-based test
  fixture calls now use explicit constructors.
- The isolated JDK-21 Gradle run of `:core-jvm:test`, filtered to
  `org.hyperledger.iroha.sdk.consensus.*` and
  `org.hyperledger.iroha.sdk.client.SumeragiHttpTransportContractTest`, passes
  **60 tests, zero failures/errors/skips** with JDK 8 API enforcement. This
  includes three new Java consumer cases against Kotlin, grouped Native
  fixtures, wire fixtures, endpoint separation and immutable ownership.
- Nightly/PR formal workflows and the standalone launcher canonicalize `/tmp`
  before creating private invocation roots. Four tests execute the actual
  setup blocks twice and pass; 27 formal-launcher tests pass. The complete
  proof-ledger test collection fails on an existing include-manifest count
  assertion, so no whole-ledger pass is claimed.
- Added six direct Native role/helper adversarial tests and one WAL crash
  regression covering both Prepare and Commit with two fresh reopens. The
  crash seam exists only under `cfg(test)`, after the fsynced append and before
  live Sign publication. Rust formatting passes on the changed test/source
  files. Rust execution and workspace formatting wait for existing Cargo/rustc
  jobs; no process was interrupted and no Rust test pass is claimed.
- M1–M6 and every release gate remain open. The full legacy Java consumer
  migration, source-inventory regeneration, formal engines, runtime corridors
  and scaling qualification remain outstanding.
- The legacy-codec guard and scoped whitespace checks pass. No new crate,
  dependency, manifest/lock change or compatibility path was introduced.
