---
title: SoraFS V1 Closure Ledger
summary: Canonical implementation, validation, documentation, and rollout-evidence ledger for the first SoraFS production release.
---

# SoraFS V1 Closure Ledger

This is the single closure index for the first SoraFS production release. It
does not replace the normative protocol contract in
`docs/source/sorafs_architecture_rfc.md`, the deployment sequence in
`docs/source/sorafs/migration_roadmap.md`, or the historical record in
`docs/source/sorafs/migration_ledger.md`. It connects those authorities to the
current implementation, tests, documentation, and external promotion evidence.

Repository conformance and production readiness are deliberately separate. A
row marked `local-complete` has a reviewed implementation and local validation
surface. A row marked `evidence-pending` still requires genuine output from the
reviewed production deployment. A row marked `open` has implementation work
remaining. `cutover-only` is reserved for Taira or Minamoto mutation, which is
outside V1 qualification and requires separate operator authorization.

The production aggregate is authoritative for promotion. It must recognize
exactly the 17 lanes in this ledger, accept one trusted signed foundational
envelope containing the nine ordered prerequisite IDs, and emit
`status=ready`, `summary_file_count=17`, and
`recognized_summary_count=17`. Documentation, canary builders, dry runs, and
synthetic fixtures cannot override a blocked aggregate.

## Authority and shared contract

| ID | Contract | Implementation and removal boundary | Validation | Rollout evidence | State |
|----|----------|-------------------------------------|------------|------------------|-------|
| V1-C01 | One canonical V1 Norito wire/state/API surface; pre-release formats are discarded rather than migrated. | `crates/sorafs_manifest`, `crates/iroha_data_model`, `crates/iroha_torii`, SDK request builders; compatibility branches must not become release inputs. | Canonical fixture regeneration, codec guards, OpenAPI parity, cross-SDK positive and negative vectors. | Signed release manifest and clean-consumer canaries. | open |
| V1-C02 | Production policy and behavior come from `iroha_config`; file keys and environment overrides are limited to explicit development/test paths. | All SoraFS daemons, Torii routes, publishers, workers, and deployment bundles. | Config parsing/default tests plus static production-path guards. | Reviewed production config digest with runtime-secret provenance. | open |
| V1-C03 | Canonical envelopes, bounded inputs, finalized cursors, idempotency, durable outboxes/dead letters, signer rotation/revocation, payload-free logs, and deterministic integer/fixed-point computation. | Common service and ledger boundaries in `crates/iroha_core`, `crates/iroha_torii`, `crates/sorafs_node`, and `crates/sorafs_manifest`. | Unit/property/model, replay, crash, poison, allocation, timestamp, signer, and concurrency negatives. | Four-validator recovery and disaster-recovery records. | open |
| V1-C04 | Orderbook, reserve/rent, repair, and moderation authority is committed ledger state; daemons reconcile rather than own competing truth. | Native instructions, queries, committed events, Torii projections, and SDK builders. The local-authority removals are listed below. | Atomicity, conservation, uniqueness, finality, fork/retry, and cross-peer duplicate-submission suites. | Identical post-recovery queries, balances, roots, events, and serialized responses on four validators. | open |
| V1-C05 | Release artifacts are reproducible, mandatory Linux/macOS x86_64/aarch64 binaries are smoke-tested, signing is Ed25519-only, and provenance/SBOM/vulnerability results are verified. | The release workflow closes the exact five-target inventory (mandatory Linux/macOS x86_64/aarch64 plus additional Windows x86_64), replays both inner validator and outer platform archives, uses locked builds, requires byte-identical shared files, and transactionally publishes Ed25519 signature/public-key/receipt outputs with no-follow exclusive creation and signer/manifest snapshots. | Release automation, archive replay, exact version-map/inventory, unsafe permission/hardlink/symlink/signer/key/signature/receipt-race negatives, shell Ed25519 integration, action-pin, and workflow static guards are green locally. | Run the five native host builds and smokes, HSM-backed reference signature, Syft/Grype scans, OIDC/cosign provenance, package-channel canaries, and prove zero critical/high findings. | open |
| V1-C06 | Promotion uses an explicit clock and one reviewed deployment context; secret material and payload data never enter evidence. | `scripts/run_sorafs_production_readiness.py`, `scripts/check_sorafs_production_readiness.py`, lane checkers, and the private-key-free two-phase builder in `scripts/build_sorafs_foundational_prerequisite.py`. Later sequences require the exact immediately preceding signed envelope; all signing inputs, the 17 reviewed lane-summary files, and output parents are pinned against path replacement. | Builder-to-aggregate acceptance, aggregate positive/replay, plus missing, duplicate, stale, predecessor, signature, path-swap, and sensitivity negatives. | One external-HSM-signed monotonic foundational envelope and 17 fresh lane summaries. | local-complete; evidence-pending |
| V1-C07 | The promotion decision is cryptographically bound to the exact reviewed lane summaries and their payload-free evidence. | The private-key-free builder hashes all 17 exact ready summary files in canonical aggregate order into the HSM-signed foundational envelope. A full promotion run rehashes the supplied summary bytes and rejects substitution. Each validated lane summary schema-closes the ordered artifact identities and SHA-256 claims reviewed by the signer. | Forged-summary, swapped-summary bytes, omitted/reordered lane, wrong-key, predecessor, path-identity, and deterministic-replay negatives. | Trusted signature and archive digest for the exact accepted aggregate input set and result. | local-complete; evidence-pending |

The release-automation dependency contract is owned by
`scripts/requirements.txt` and
`scripts/tests/check_sorafs_release_automation_test.py`. The aggregate contract
and its 14-day default freshness ceiling are owned by
`scripts/check_sorafs_production_readiness.py`; the runner requires an explicit
`--now-unix` so a reviewed dry run and its execution cannot silently use
different clocks.

### Chunk fetch-plan interchange closure

The V1 fetch-plan boundary has one standalone representation. Every producer
and consumer listed below uses the exact `sorafs.chunk_fetch_plan.v1` object
with `schema`, a non-zero canonical lowercase
`payload_digest_blake3_hex`, and `chunk_fetch_specs`. The digest binds the
complete unchunked payload. Bare arrays, missing or zero bindings, unsupported
root fields, digest substitution, and reconstructed-payload mismatches fail
closed; there is no legacy compatibility branch.

| Classification | Surfaces | V1 rule |
|----------------|----------|---------|
| Standalone interchange | `sorafs_cli` CAR/deploy/governance outputs and fetch/proof inputs; `sorafs_manifest_builder`; `sorafs_chunk_store`; `sorafs-node`; Torii DA `chunk_plan`; `iroha app sorafs fetch`; Rust local fetch; JavaScript, Python, and Connect/native bridges; provider-admission, orchestrator-parity, and CI fixtures. | Read and write only the strict payload-bound `sorafs.chunk_fetch_plan.v1` object. Reassembly must reproduce its exact whole-payload BLAKE3 digest. |
| Typed embedded field | `sorafs.manifest_builder_report.v1`, `sorafs.toolkit_pack_report.v1`, and `sorafs.chunk_store_report.v1`. | `chunk_fetch_specs` may appear only after the containing report schema and required V1 metadata have been validated. The selected field is never accepted as a standalone plan. |

## Foundational prerequisite envelope

The signed envelope order is fixed by
`scripts/check_sorafs_production_readiness.py`. Every row must be `verified`, use
the same reviewed production deployment and active anchors as the lane
summaries, include a fresh evidence anchor, and be signed by a trusted runtime
Ed25519 key with a monotonic release sequence and the required predecessor.
The preparation and external-HSM finalization procedure is documented in
`docs/source/sorafs/foundational_prerequisite_signing.md`; it never accepts
private signing material or creates a checked-in production envelope.

| Order | ID | Canonical scope | Local proof surface | Required external proof | State |
|-------|----|-----------------|---------------------|-------------------------|-------|
| 1 | SFM-1 | Deterministic multi-source routing and authority join. | The canonical bounded join lives in `sorafs_orchestrator`, is keyed by exact finalized height/hash, rebuilds single-flight, rejects stale/fork identities without fallback, and emits canonical byte-identical Norito projections under fixed aggregate/output caps. Routing fixtures, replica-order parity, concurrency, eviction, failure, and authority tests are present. | Byte-identical multi-provider results and two-region failover/load evidence. | evidence-pending |
| 2 | SF-1 | Canonical manifest, registry, provider advert, alias, and retrieval contract. | Manifest/registry data model, Torii admission, fixture and SDK guards. | Final pin/alias/provider/retrieval receipts from the reviewed deployment. | evidence-pending |
| 3 | SF-2 | PDP provider protocol and proof validation. | Canonical PDP schemas, the durable provider protocol, authenticated Torii challenge/next-work/proof/status/terminal-export family, fail-closed exact-chain native repair-transaction handoff, finalized reconciliation, and storage execution gated by the exact finalized lease are implemented locally. The competing local repair projection is deleted. | Authenticated deployed provider transport, multi-provider proof replay, cross-peer exactly-once repair, and observability. | open |
| 4 | SF-2c | PoR/PoTR challenge, receipt, and repair convergence. | PoR/PoTR schemas, coordinators, reference validation, final dual-signed PoTR receipt persistence, 32-byte native repair task identities, fail-closed exact-chain native handoff, finalized-lease-gated storage execution, exactly-once latency-repair identity, PoR latency/VRF/seed metrics, and the local repair-authority removal are implemented. | Live provider/auditor runs, governed provider-key rotation, dual-signed durable receipts, cross-peer exactly-once repair, and archive evidence. | open |
| 5 | SF-3 | Gateway serving and compliance boundary. | Gateway conformance, policy/catalog helpers, load and compliance checkers. | Two independently administered gateways, live catalogs, failover, security probes, and load/soak. | open |
| 6 | SF-4 | Repair authority and lifecycle. | Native task/lease/terminal/slash/appeal instructions, caller-signed one-instruction transaction ingress, finalized queries/events, exact-chain durable transaction forwarding, full-projection GC/reconciliation, and storage execution bound to the exact live finalized lease are implemented. The public `RepairManager`, filesystem store/checkpoint, local mutation/query/event APIs, and compatibility paths are deleted. | Prove cross-peer exactly-once execution and one terminal outcome in the reviewed deployment. | open |
| 7 | SF-5b | Multi-provider range streaming and proof-aware delivery. | Deterministic gateway suite, SDK orchestrator, range/proof validation. | At least 1,000 concurrent live streams in cold/warm/mixed profiles with injected failures. | evidence-pending |
| 8 | SF-6 | Governance, economics, and settlement integration. | Local governance payloads, orderbook/reserve/finance models, and lane gates. | Atomic committed-state settlement, finance, governance, and recovery evidence. | open |
| 9 | SF-8a | Moderation, credentials, evidence access, and transparency integration. | Local moderation/PoP/transparency primitives and evidence contracts. | Deployed orchestration, protected viewer, HSM/WebAuthn flows, public proofs, and privacy-budget evidence. | open |

No production foundational envelope is checked into the repository. Runtime
signing keys, HSM credentials, and predecessor state remain external release
inputs.

## Readiness lanes

The required-kind inventories below are executable contracts imported by the
aggregate checker. There are 135 required kinds across 17 summaries.

| Lane | Required kinds | Canonical plan and implementation anchors | Local verification | Closure requirement | State |
|------|---------------:|-------------------------------------------|--------------------|---------------------|-------|
| `ai_prescreen` | 8 | `docs/source/sorafs_ai_prescreen_plan.md`; config-pinned canonical screening authority, signed-result/exact-committee Torii admission, chunked ChaCha20-Poly1305 quarantine envelopes, runtime PKCS#11/KMS wrapper boundary, and `scripts/check_sorafs_ai_prescreen_rollout_evidence.py`. | Authority digest/canonical/path bounds, signer/quorum/policy/manifest/binding/freshness/replay negatives; AEAD wrong key/tag/AAD/chunk/range/recovery/rewrap and payload-redaction tests; checker, runner, builder, and aggregate binding. | Resolve `V1-BLOCK-AI-QUARANTINE-KMS-01`, then deploy the trained committee workflow and KMS-backed quarantine service, exercise recovery/rotation, publish DAG/transparency records, and collect end-to-end evidence. | open |
| `appeal_finance` | 10 | `docs/source/sorafs_appeal_pricing_plan.md`; Torii pricing/settlement surfaces and finance governance payloads. | Quote/deposit/settlement/config binding and aggregate tests. | HSM submitter, durable finalized cursor, reconciliation, dashboard/alerts, and four-peer settlement evidence. | open |
| `gateway_compliance` | 10 | `docs/source/sorafs_gateway_compliance_plan.md`; signed predecessor-bound controller core, bounded runtime feed-transport contract, atomic checkpoint/LKG promotion and rollback, scoped toggle/appeal/hold precedence, canonical region/gateway identities, six authenticated Torii control routes, `AppState`-retained controller/transport, live manifest/CID/provider enforcement, unsigned-bootstrap mutual exclusion, ACME fail-closed source guard, bounded `torii_sorafs_gateway_compliance_*` telemetry with dashboard/alerts, and compliance evidence gate. | Gateway controller signature/predecessor/quorum/precedence/persistence and SSRF/DNS-rebinding/pin/redirect/decompression negatives; canonical-body/auth/role/error mapping, serving-scope/fail-closed/bootstrap-exclusion tests; bounded-label telemetry and dashboard/alert static contracts; TLS runtime-contract static guard; feed, policy, catalog, payload-safety, binding, runner, and aggregate negatives. | `V1-BLOCK-GATEWAY-CONTROLLER-RUNTIME-01`: independently audited standard-daemon feed and ACME adapters, finalized appeal/hold catalog producers, external threshold signing, two-gateway deployment/promotion/rollback, and genuine probes. | open |
| `gateway_load` | 5 | `docs/source/sorafs_gateway_load_tests.md`; deterministic 1,000-stream conformance harness. | Local conformance, staged-load schema, SLO, transport-scope, and aggregate tests. | Live cold/warm/mixed multi-provider runs, corruption/failover/flood pressure, and 24-hour soak. HTTP/3 is not applicable to V1. | evidence-pending |
| `governance_dag` | 8 | `docs/source/sorafs_governance_dag_plan.md`; `sorafs_governance_dag`, bounded authenticated JSON mirror, IPFS/IPNS publication, checkpoint, health, and metrics. | Service/unit/adversarial tests, rollout checker/runner, mock-IPFS and opt-in Kubo lane. | Package/supervise two instances, governed credentials, CAS/failover, public mirror, alert-routing, rollback, and recovery evidence. RocksDB/IPLD is conditional on the SF-12 capacity gate. | local-complete; evidence-pending |
| `hedging_billing` | 8 | `docs/source/sorafs_hedging_plan.md`; pricing/billing payloads, native bridge, and hedging gate. | Fixture, price/cycle/policy binding, runner, and aggregate tests. | Live feeds, exposure/accrual/statements/acknowledgements/reconciliation/alerts; governed adapter shipped with automatic execution disabled. | open |
| `moderation_panel` | 12 | `docs/source/sorafs_moderation_panel_plan.md`; native moderation types, finalized-chain orchestrator and projections, continuous worker, and evidence-viewer canary. | Contiguous committed-event replay, exact sortition/selection authority, commit/reveal, restart reconciliation, duplicate cross-peer submission, no-show/failover, policy/case/roster/tally bindings, viewer safety, runner, and aggregate tests. | Complete protected-viewer and downstream settlement/publication deployment, then collect four-peer case evidence. | open |
| `orderbook` | 9 | `docs/source/sorafs_orderbook_plan.md`; canonical payloads in `crates/sorafs_manifest/src/orderbook.rs`, native policy/state in `crates/iroha_data_model/src/sorafs/orderbook.rs`, atomic execution in `crates/iroha_core/src/smartcontracts/isi/sorafs_orderbook.rs`, finalized Torii projections in `crates/iroha_torii/src/sorafs/api.rs`, and durable native transaction forwarding/reconciliation. The competing `sorafs_node` book, checkpoint, config, mutation/event surface, settlement publisher, and obsolete runtime-snapshot wire/SDK selectors are deleted. | Native admission sequence/revision/order/fill/trade/channel/escrow/expiry/refund model tests, exact governed matcher/settlement authority negatives, finalized query/cursor tests, retired-selector rejection, reference validation, checker/runner, policy/contract bindings, and aggregate tests. | Complete source/SDK validation, then collect four-peer simultaneous-submission, restart, expiry/refund race, single-settlement, and recovery evidence from the reviewed deployment. | open |
| `pdp` | 6 | `docs/source/sorafs_pdp_plan.md`; `crates/sorafs_node/src/pdp_provider.rs`, PDP manifest modules, authenticated Torii provider routes, challenge-bound proof streaming, exact-chain durable native repair transaction handoff, finalized-lease-gated execution, and deletion of the local repair projection. | Protocol/reference, malformed/canonical/replay/admission/restart, native-handoff failure/retry/reconciliation, stale/wrong lease and restart-deduplication tests, checker/runner negatives, and a static guard that requires the five-route V1 API while rejecting reserved placeholder routes. | Exercise the authenticated protocol across multiple deployed providers and collect cross-peer repair plus live metrics/archive evidence. | open |
| `pop_credentials` | 9 | `docs/source/sorafs_pop_credentials_plan.md`; native PoP registry, `crates/sorafs_node/src/pop_credentials.rs`, and the canonical authenticated 14-route Torii V1 service. | Durable encrypted enrollment/wallet, dual control, HSM/KMS interfaces, outbox/reconciliation, canonical/depth/allocation/auth/time rollback negatives, privacy, runner, and aggregate tests. | Resolve `V1-BLOCK-POP-RUNTIME-01`: package and wire the governed external-runtime/sidecar provider bundle into standard `irohad`, then exercise HSM issuance, reconciliation/revocation, wallet custody, local proofs, verifier replay defense, restart, and rotation without ledger/log secrets or PII. | open |
| `por` | 6 | `docs/source/sorafs_por_plan.md`; PoR scheduler/randomness/governance foundations, request-bound sampling, emitted latency/VRF/seed metrics, exact-chain durable native repair handoff, finalized-lease-gated execution, and deletion of the local repair projection. | VRF/seed/request replay, proof/reference, native 32-byte task identity, handoff retry/signature/checkpoint corruption, stale/wrong lease, restart deduplication, metric-label bounds, checker/runner, and aggregate tests. | Run live randomness with provider/auditor scheduling and replay, prove cross-peer exactly-once repair, archive the reporting output, and obtain governed approval. | open |
| `potr` | 6 | `docs/source/sorafs_potr_plan.md`; `crates/sorafs_node/src/potr.rs`, proof-stream/governance schemas, fail-closed exact-chain durable native repair handoff, finalized-lease-gated execution, and deletion of the local repair projection. | Final receipt/reference, gateway Ed25519 and governed provider ML-DSA-65 signer negatives, persistence/restart, native repair idempotency, stale/wrong lease, restart deduplication, PQ roster, reputation binding, checker/runner, and aggregate tests. | Exercise dual-signed receipts, key rotation, crash recovery, and cross-peer exactly-once repair in the reviewed deployment. | open |
| `reference_sdk_release` | 6 | `docs/source/sorafs_reference_sdk_plan.md`; `sorafs-validate`, Rust reference core, ABI-21 C/Node/Python/JNI/Swift/C# wrappers, Governance DAG block/head validation in all six downstream SDK families, and the release packager. | Eight complete `ValidationOutcomeV1` Governance block/head positive and negative vectors are checked exactly by JavaScript, Python, Swift, Kotlin/JVM, mirrored Java Android, and C#. A test-only Ed25519-signed closed inventory binds nine Norito payloads, eight canonical JSON sidecars, and eight outcomes; its offline checker rejects tamper, missing/extra/duplicate/traversal, noncanonical/nonfinite JSON, symlink/hardlink, and parent-swap attacks, while the Rust generator has two-run byte-identical coverage. FFI/header, archive/signature, workflow, and aggregate tests remain in place. | Run the Rust regeneration test, rebuild the checked-in Swift XCFramework at ABI 21, run Python and C# on supported runtimes, extend the signed inventory across the remaining routing/orderbook/PDP/PoR/PoTR/repair/moderation domains, then complete mandatory platform binaries, clean-consumer packages, SBOM/provenance, and published canaries. | open |
| `repair` | 8 | `docs/source/sorafs_repair_plan.md`; native repair records/ISIs/queries/events, Torii caller-signed one-instruction command ingress and finalized query routes, `crates/sorafs_node/src/repair_transaction_forwarder.rs`, the finalized native lease storage executor, bounded full-projection GC/reconciliation, and deletion of the local manager/store/checkpoint/API authority. | Native lifecycle/lease/action/appeal atomicity, exact-chain transaction signing and finality reconciliation, route-specific `202` command and `200` query responses, stale cursor/owner/generation/expiry rejection, malformed finalized-task and unsafe chunk-path rejection, restart deduplication, replay/rate-limit/event, checker/runner, binding, and aggregate tests. | Prove cross-peer exactly-once execution and one terminal outcome. | open |
| `reputation` | 8 | `docs/source/sorafs_reputation_plan.md`; deterministic reputation/reference/governance foundations; native governed journal policy/history, one global sequence, event/source indexes, typed committed events, fixed-view query, PoR/token append instructions, and atomic capacity-dispute `Opened`/`Resolved` integration; plus the finalized-identity-keyed, single-flight, byte-identical SFM-1 authority join/cache in `sorafs_orchestrator`. | Journal policy predecessor/rotation, exact permission and recorder authority, source/provider/policy/block-time binding, global/source continuity, replay, forged/orphan state, bounded fixed-view pagination, and atomic dispute lifecycle tests; snapshot/consumer determinism, join concurrency/stale/fork/bounds/replica parity, transport/checker/runner, and aggregate tests. Integrated source validation remains pending. | Add authenticated Torii/OpenAPI and SDK transaction/query projections; connect PDP/PoTR, repair, orderbook/settlement, and reserve/rent committed producers; deploy the durable finalized-cursor ingest/reconciliation/outbox service and external threshold-signing flow; then collect four-peer and production evidence. The existing local snapshot checkpoint is not the committed projection. | open |
| `reserve_rent` | 11 | `docs/source/sorafs_reserve_rent_plan.md`; native reserve policy/provider/movement/rent/lifecycle/credit/repayment/appeal records and ISIs; exact caller-signed one-instruction Torii mutation ingress; authenticated finalized policy/provider/movement/appeal/event projections; durable native forwarding workers; finalized-event/provider telemetry with bounded labels, reconciliation readiness, and represented height. The process-local `sorafs_node` reserve runtime, checkpoint, scheduler, mutation API, obsolete routes, and telemetry authority are deleted. | Native accounting/conservation/authority/lifecycle/event/query tests; signed-envelope, route/instruction/authority/policy/revision/cursor/authentication negatives; empty-page event-resume and atomic telemetry-rebuild regressions; Prometheus rule tests; fresh scrape digest/reconciled-height evidence negatives; matrix/ledger/policy binding, checker/runner, and aggregate tests. | Complete source validation, then prove custody, fork, retry, signer/failover, projection rebuild, and peer reconciliation on the reviewed deployment. | open |
| `transparency` | 5 | `docs/source/sorafs_transparency_plan.md`; canonical fixed-population/fixed-metric public aggregates with retired exact source fields rejected; exact integer discrete-Laplace sampling; stable query/window release identity; private tagged source digests; durable hash-chained composition-budget and release ledgers; atomic source deletion/outbox persistence; finalized-head reconciliation; and runtime-only threshold-PRF/release-anchor injection boundaries in `crates/sorafs_manifest/src/transparency.rs`, `crates/sorafs_node/src/transparency.rs`, `crates/sorafs_node/src/lib.rs`, Torii, config, and `irohad`. | Public-schema rejection, clipping, fixed-bucket DP+k, joint-sensitivity, sampler, PRF binding/redaction, release-chain tamper, budget, checkpoint rollback/equivocation, source-ingest response, scheduler, config, checker/runner, and aggregate tests. | Implement and deploy independently administered production `PrivacyCyclePrfProviderV1` and `PrivacyReleaseAnchorV1` adapters; connect every finalized producer; add leader lease/HSM Governance DAG anchoring, public replicas/proofs/pagination/ETags and hardened explorer delivery; then capture multi-replica, differencing/budget-exhaustion, failover, rollback, and genuine rollout evidence. | open |

## Active local release blockers

### V1-BLOCK-AI-QUARANTINE-KMS-01 — standard-daemon quarantine key adapter

The signed screening boundary, config-pinned canonical authority bundle,
chunked ChaCha20-Poly1305 envelopes, per-object DEKs, authenticated range
decryption, atomic recovery, rewrap, Torii runtime dependency, and
`Iroha::start_with_runtime_deps` handoff are present. Enabling authenticated
screening without a runtime `ModerationQuarantineKeyWrapper` now fails closed at
node startup.

The standard `irohad` entrypoint intentionally injects no wrapper, and the
repository contains no deployable PKCS#11 or managed-KMS provider
implementation/factory. This blocker closes only when the reference launcher
constructs that adapter from runtime-only credentials, binds its non-secret key
handle to the reviewed deployment, and passes provider-outage, config/runtime
mismatch, wrong key/tag/AAD, chunk reorder, range, restart recovery,
rotation/rewrap, rollback, concurrency, and payload-free log/API tests. A
tests-only wrapper, local file key, environment key, software wrapping fallback,
or committed credential cannot satisfy the gate.

### V1-BLOCK-POP-RUNTIME-01 — standard-daemon PoP provider adapter

The PoP issuer/wallet service, strict `iroha_config` policy, authenticated
Torii routes, and runtime injection seam are present. The standard `irohad`
entrypoint does not yet construct `PopCredentialRuntimeSecretsV1`, and the
repository does not contain a deployable shared external-runtime or sidecar
adapter for its enrollment/wallet hybrid secrets, governed issuer HSM, KMS key
wrapper, API authenticator, registry submitter/reader, private issuance/witness
providers, and finalized-time provider. Enabling PoP without that bundle fails
startup by design.

This blocker closes only when the shared external-runtime lane packages the
adapter, binds its non-secret handles/public keys to the exact production
config, wires it into standard `irohad`, and passes provider-outage,
config/runtime mismatch, key/time rotation, rollback, restart reconciliation,
and four-validator reference-deployment tests. No file-key, environment-key,
software-signing, process-clock, or tests-only provider may satisfy the gate.
The operator procedure and required proof are maintained in
`docs/source/sorafs_pop_credentials_plan.md` under “Runtime Adapter Blocker
Runbook.”

## Competing local authority removal

These local components may remain as caches, projections, deterministic test
models, or development fixtures only after their authoritative transition moves
to committed state. They must not be accepted as production truth.

| Domain | Competing local authority | V1 replacement | Removal proof |
|--------|---------------------------|----------------|---------------|
| Orderbook | Local book/matcher state, local revision, and locally final settlement receipts. | Native order/fill/trade/channel/escrow/expiry/refund instructions, queries, and committed events. | Restart/cross-peer model tests and identical committed projections. |
| Reserve/rent | Local lifecycle, movement, custody, credit, debt, appeal, and pricing checkpoints. | Atomic authority-checked native records and finalized queries/events. | Conservation/underflow/double-settlement/fork tests and balance reconciliation. |
| Repair | Process-local task identity, checkpoint ownership, or filesystem/cross-process lock as final authority. | Chain task identity, lease, terminal outcome, slash, and appeal state. | Duplicate-claim/restart/partition tests with one terminal outcome. |
| Moderation | Process-local ballot, appeal, scheduler, or settlement state. | Native moderation ledger plus rebuildable finalized-chain projections. | Crash/rebuild/fork/no-show/failover tests with no local-only transition. |
| PoTR | In-memory receipt tracker or optional production signatures. | Atomically persisted final dual-signed receipt and exactly-once repair identity. | Crash-before/after-rename, duplicate/reordered receipt, signer, and latency-repair tests. |
| Pricing | Local governed pricing checkpoint used without transaction forwarding and finality reconciliation. | Signed feed input followed by committed policy/settlement state and durable cursor/outbox. | Feed rollback, stale price, duplicate accrual, and peer reconciliation tests. |
| Reputation ingest | Governance DAG, telemetry exporters, or a local event database accepted as source authority. | Governed native journal plus typed finalized domain events; the service database is a rebuildable projection with a durable cursor/outbox. | Policy/source/replay negatives, projection rebuild, cross-peer identical signing material, restart reconciliation, and no local-only transition. |

## Test, security, and operations closure

| Gate | Repository command or owner | Promotion proof | State |
|------|-----------------------------|-----------------|-------|
| Formatting/build/lint | `cargo fmt --all -- --check`; `cargo build --workspace --locked`; strict workspace Clippy. | Logs from the pinned release commit. | validation-pending |
| Workspace and SDK tests | `cargo test --workspace --locked`; script suites; Swift, Kotlin/JVM, Java Android, JavaScript, Python, and C# workflows. | Full positive/negative parity matrix. | validation-pending |
| Fixtures/OpenAPI | SoraFS fixture guards, double regeneration, clean pinned Torii OpenAPI generation, source/version digest checks. | Byte-identical outputs and signed manifest. | open |
| Adversarial/security | Fuzz/model/property suites plus dependency, SBOM, provenance, and vulnerability scans. | No unresolved blocker and no critical/high finding. | open |
| Distributed/load/soak | Four voting validators with DA/RBC, multiple providers, two regional gateways, partitions/restarts/rotations; at least 1,000 concurrent streams; 24-hour soak. | Payload-free reports bound to the final deployment and active policy/digest anchors. | evidence-pending |
| Disaster recovery | Backup restore, signer/root rotation, gateway/DAG failover, rollback/yank rehearsal. | Reviewed recovery receipts with rollback still available. | evidence-pending |
| Source-marker and competing-authority audit | `scripts/tests/check_sorafs_rollout_gate_contract_test.py` plus semantic review of production handlers, state stores, and shipped binary names. | Zero active deferred-work markers, placeholder binaries, default “not implemented” production methods, or process-local authorities at the release commit. | open |

## Promotion and cutover

The aggregate checker is run once for promotion and again with identical inputs
for deterministic replay. Missing, duplicate, stale, tampered, predecessor, and
signature-negative runs are archived alongside it. Promotion is permitted only
after every row above is either `local-complete` or `evidence-pending` with its
required evidence supplied and accepted, the foundational envelope is valid,
all 17 lane summaries are fresh and valid, and the aggregate reports ready
with no errors.

Taira and Minamoto mutation are `cutover-only`. The V1 evidence bundle prepares
an operator-controlled promotion but does not authorize either live cutover.
