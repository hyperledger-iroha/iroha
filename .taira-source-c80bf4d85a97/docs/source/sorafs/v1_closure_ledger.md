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
| 2 | SF-1 | Canonical manifest, registry, provider advert, alias, storage-ingest, and retrieval contract. | Manifest/registry data model, Torii admission, fixture and SDK guards, plus the opt-in supervised finalized-ledger provider-ingest worker with a bounded immutable assignment snapshot, durable single-writer completion outbox, exact fee-quoted transaction construction, committed reconciliation, monotonic recovery, and fail-closed liveness. | Resolve `V1-BLOCK-PROVIDER-INGEST-RUNTIME-01`, then collect final pin/alias/provider/ingest/retrieval receipts from the reviewed deployment. | open |
| 3 | SF-2 | PDP provider protocol and proof validation. | Canonical PDP schemas, the durable provider protocol, authenticated Torii challenge/next-work/proof/status/terminal-export family, fail-closed exact-chain native repair-transaction handoff, finalized reconciliation, and storage execution gated by the exact finalized lease are implemented locally. The competing local repair projection is deleted. | Authenticated deployed provider transport, multi-provider proof replay, cross-peer exactly-once repair, and observability. | open |
| 4 | SF-2c | PoR/PoTR challenge, receipt, and repair convergence. | PoR/PoTR schemas, coordinators, reference validation, final dual-signed PoTR receipt persistence with an exact finalized admission-policy binding, 32-byte native repair task identities, fail-closed exact-chain native handoff, finalized-lease-gated storage execution, exactly-once latency-repair identity, PoR latency/VRF/seed metrics, `PotrStateFinalizedPolicySourceV1`, `PotrFinalizedAdmissionReaderV1`, and the local repair-authority removal are implemented. | Live provider/auditor runs through the production finalized reader, governed provider-key rotation, dual-signed durable receipts, cross-peer exactly-once repair, and archive evidence. | open |
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
| `appeal_finance` | 10 | `docs/source/sorafs_appeal_pricing_plan.md`; deterministic pricing and plan-only settlement APIs, native escrow projections, bounded durable semantic transaction outbox/cursor/dead-letter state, supervised finalized-chain reconciliation, runtime-only opaque Ed25519 signer bindings with activation/revocation, strict returned-transaction verification, and post-finality Governance DAG receipts. `iroha_config` accepts no appeal-finance private key, and standard `irohad` does not derive a provider from its node key. | Quote/deposit/settlement/config binding and aggregate tests; source-level adversarial tests cover signer substitution, wrong authority/signature, rotation/revocation boundaries, duplicate/substitution replay, crash recovery, retry exhaustion, stale fork/cursor, reconciliation, and poisoned checkpoints. Focused Rust execution remains pending while the shared build lane is occupied. | Resolve `V1-BLOCK-APPEAL-SETTLEMENT-DEPLOYMENT-01`: run focused/workspace validation, inject a genuinely administered PKCS#11/HSM/KMS provider into the reference launcher, exercise outage/rotation/restart/fork and four-peer exactly-once lock/refund/disbursement, validate alert routing, and collect signed payload-free deployment evidence. | open |
| `gateway_compliance` | 10 | `docs/source/sorafs_gateway_compliance_plan.md`; signed predecessor-bound controller core, bounded runtime feed transport, process-lifetime stable-file lease, revisioned exact-byte compare-and-swap checkpointing, same-directory atomic replace with file/parent sync and post-write fencing, LKG promotion/rollback, scoped toggle/appeal/hold precedence, canonical region/gateway identities, six authenticated Torii control routes, `AppState`-retained controller/transport, live manifest/CID/provider enforcement, canonical HTTP 451 `gateway_compliance_denied` serving contract and fail-closed 503 unavailable contract, unsigned-bootstrap mutual exclusion, ACME fail-closed source guard, bounded `torii_sorafs_gateway_compliance_*` telemetry with dashboard/alerts, and compliance evidence gate. The Torii/config local store, routes, CLI/xtask mutation commands, CI helper, sample bundle, legacy `sorafs_car` denial/proof wire, and readiness/self-cert `denylist_*` inventories are removed. The gate now binds canonical catalog promotion/predecessor/precedence evidence, exact observed `451` probes, and acknowledgements from distinct gateway, region, and administrator identities. | Gateway controller signature/predecessor/quorum/precedence/persistence, dual-controller lease, restart, stale-CAS, pre-persist conflict, crash-before/after-replace, revision rollback, truncation, symlink/hardlink, and unsafe-permission negatives; SSRF/DNS-rebinding/pin/redirect/decompression negatives; canonical-body/auth/role/error mapping, serving-scope/451/503/fail-closed/bootstrap-exclusion tests; bounded-label telemetry and dashboard/alert static contracts; TLS runtime-contract static guard; feed, policy, catalog, payload-safety, binding, runner, aggregate, legacy-field/code rejection, distinct-administrator, and observed-probe negatives. | Resolve `V1-BLOCK-GATEWAY-CONTROLLER-RUNTIME-01`: independently audited standard-daemon feed and ACME adapters, finalized appeal/hold catalog producers, external threshold signing, two-gateway deployment/promotion/rollback, and genuine probes. | open |
| `gateway_load` | 5 | `docs/source/sorafs_gateway_load_tests.md`; deterministic 1,000-stream conformance harness. | Local conformance, staged-load schema, SLO, transport-scope, and aggregate tests. | Live cold/warm/mixed multi-provider runs, corruption/failover/flood pressure, and 24-hour soak. HTTP/3 is not applicable to V1. | evidence-pending |
| `governance_dag` | 8 | `docs/source/sorafs_governance_dag_plan.md`; `sorafs_governance_dag`, bounded authenticated JSON mirror, IPFS/IPNS publication, checkpoint, health, and metrics. The embedded publisher now accepts only an injected runtime signer bound to an opaque handle, peer identity, and canonical non-weak Ed25519 public key. The service exposes a supervised library launcher requiring rotation-aware IPFS/head authenticators and a sealed monotonic CAS checkpoint store; all former signing-key, checkpoint-key, and bearer-token path settings are removed and the provider-free packaged launcher fails closed. | Service/unit/adversarial tests cover missing/mismatched/drifting providers, malformed/weak keys, bad signatures, provider-error redaction, credential rotation/outage, sealed-state tamper/CAS/rollback/replay/deletion, and legacy secret-path rejection without following symlinks; rollout checker/runner, mock-IPFS, and opt-in Kubo lane remain. | Finish `V1-BLOCK-GOVERNANCE-DAG-RUNTIME-01`: implement deployment-owned HSM signer, authenticator, and sealed-store adapters; package/supervise two instances with governed credentials, CAS/failover, public mirror, alert-routing, rollback, and recovery evidence. RocksDB/IPLD is conditional on the SF-12 capacity gate. | open |
| `hedging_billing` | 8 | `docs/source/sorafs_hedging_plan.md`; canonical feed/pricing/billing payloads and bridge; durable signed-feed high-water state; bounded finalized event/period-close ingest; deterministic accrual, governed statement, aggregate exposure, and hedge-intent projection; durable signing/publication/acknowledgement/reconciliation/dead-letter state; sealed epoch witnesses; a supervised `irohad` worker with strict non-secret `iroha_config`, opaque identity-pinned finalized-query/journal-verifier/HSM-signer/publisher/acknowledgement/witness adapter seams, payload-free health/alert metrics, and no automatic-execution loop; plus seven canonical-account-authenticated, bounded, private/no-store billing, reconciliation, exposure, and intent routes backed only by the committed projection. Every intent disables automatic execution, and the separately authorized submission helper rejects automatic adapters. | Fixture, price/cycle/policy binding, route/catalog/OpenAPI authentication and schema contracts, runner, and aggregate gates are present. Focused Rust verification of the new service/runtime remains pending. | Deploy live external feeds and genuine finalized-query, journal-verifier, HSM/KMS signer, immutable publisher, acknowledgement-authority, sealed-witness, and any manually governed venue adapters; deploy and exercise the shipped API and add runtime CLI clients; validate scrapes and alert routing; then collect staged billing/reconciliation and rollout evidence. Automatic execution remains disabled. | open |
| `moderation_panel` | 12 | `docs/source/sorafs_moderation_panel_plan.md`; native bounded moderation ISIs/queries/events, finalized-chain orchestrator, exact retained transaction bytes, ambiguous-ingress reconciliation, notification leases, terminal handoff contracts, fresh worker-owned read projection, deadline/non-overlap/freshness supervision, and the signed evidence-viewer checkpoint plus exact predecessor-bound receipt projection as the sole audit authority. The retired aggregate-audit POSTs are authenticated/authorized `410 Gone` tombstones and have no scheduler or local registry mutation. | Contiguous committed-event replay, exact sortition/selection authority, commit/reveal, restart reconciliation, duplicate cross-peer submission, no-show/failover, policy/case/roster/tally bindings, hung/stale/dead-letter/cursor-equivalence supervision, viewer authorization/WebAuthn/grant/range/receipt/projection-cursor/hold/erasure safety, runner, and aggregate tests. Focused Rust validation of the latest hardening remains pending. | Resolve `V1-BLOCK-MODERATION-VIEWER-RUNTIME-01`: construct production runtime providers; add durable notification delivery plus settlement/publication and signed receipt-to-transparency adapters; add operation-ID and monotonic multi-instance checkpoint fencing; add signed replay-safe compaction/archive; then deploy real HSM/KMS/WebAuthn/downstream adapters and collect four-peer case evidence. | open |
| `orderbook` | 9 | `docs/source/sorafs_orderbook_plan.md`; canonical payloads in `crates/sorafs_manifest/src/orderbook.rs`, native policy/state in `crates/iroha_data_model/src/sorafs/orderbook.rs`, atomic execution in `crates/iroha_core/src/smartcontracts/isi/sorafs_orderbook.rs`, finalized Torii projections in `crates/iroha_torii/src/sorafs/api.rs`, durable native transaction forwarding/reconciliation, and the supervised finalized-ledger provider-ingest completion worker. The competing `sorafs_node` book, checkpoint, config, mutation/event surface, settlement publisher, and obsolete runtime-snapshot wire/SDK selectors are deleted. | Native admission sequence/revision/order/fill/trade/channel/escrow/expiry/refund model tests, exact governed matcher/settlement authority negatives, finalized query/cursor tests, provider-ingest retry/restart/tombstone/finality/identity negatives, retired-selector rejection, reference validation, checker/runner, policy/contract bindings, and aggregate tests. | Complete source/SDK validation, resolve `V1-BLOCK-PROVIDER-INGEST-RUNTIME-01`, then collect four-peer simultaneous-submission, provider-ingest restart/rotation, expiry/refund race, single-settlement, and recovery evidence from the reviewed deployment. | open |
| `pdp` | 6 | `docs/source/sorafs_pdp_plan.md`; `crates/sorafs_node/src/pdp_provider.rs`, PDP manifest modules, authenticated Torii provider routes, challenge-bound proof streaming, exact-chain durable native repair transaction handoff, finalized-lease-gated execution, and deletion of the local repair projection. | Protocol/reference, malformed/canonical/replay/admission/restart, native-handoff failure/retry/reconciliation, stale/wrong lease and restart-deduplication tests, checker/runner negatives, and a static guard that requires the five-route V1 API while rejecting reserved placeholder routes. | Exercise the authenticated protocol across multiple deployed providers and collect cross-peer repair plus live metrics/archive evidence. | open |
| `pop_credentials` | 9 | `docs/source/sorafs_pop_credentials_plan.md`; native PoP registry, `crates/sorafs_node/src/pop_credentials.rs`, and the canonical authenticated 14-route Torii V1 service. | Durable encrypted enrollment/wallet, dual control, HSM/KMS interfaces, outbox/reconciliation, canonical/depth/allocation/auth/time rollback negatives, privacy, runner, and aggregate tests. | Resolve `V1-BLOCK-POP-RUNTIME-01`: package and wire the governed external-runtime/sidecar provider bundle into standard `irohad`, then exercise HSM issuance, reconciliation/revocation, wallet custody, local proofs, verifier replay defense, restart, and rotation without ledger/log secrets or PII. | open |
| `por` | 6 | `docs/source/sorafs_por_plan.md`; PoR scheduler/randomness/governance foundations, request-bound sampling, emitted latency/VRF/seed metrics, exact-chain durable native repair handoff, finalized-lease-gated execution, and deletion of the local repair projection. | VRF/seed/request replay, proof/reference, native 32-byte task identity, handoff retry/signature/checkpoint corruption, stale/wrong lease, restart deduplication, metric-label bounds, checker/runner, and aggregate tests. | Run live randomness with provider/auditor scheduling and replay, prove cross-peer exactly-once repair, archive the reporting output, and obtain governed approval. | open |
| `potr` | 6 | `docs/source/sorafs_potr_plan.md`; `crates/sorafs_node/src/potr.rs`, proof-stream/governance schemas, fail-closed exact-chain durable native repair handoff, finalized-lease-gated execution, deletion of the local repair projection, and Torii's injected `PotrRuntimeSignersV1` boundary. The stream-token issuer no longer owns or derives a provider ML-DSA key. Distinct gateway/provider runtime objects and administrative identities are mandatory. Torii constructs `PotrFinalizedAdmissionReaderV1` from `PotrStateFinalizedPolicySourceV1` and the council-verified admission registry, resolves the exact live policy before every receipt, and rechecks it after both signatures; the startup registry alone is not authorization. The tracker atomically persists provider, policy identity/digest/sequence, finalized height/hash, and exact admission-envelope digest, and exposes that retained anchor as the floor for the next read. | Final receipt/reference, signature-shape, persistence/restart, native repair idempotency, stale/wrong lease, restart deduplication, PQ roster, reputation binding, checker/runner, and aggregate tests; source tests cover shared/drifting signer and reader identities, wrong role/provider/key/algorithm, inactive or untrusted admission, partial invalid output, reader/signer outage, revocation, stale and same-sequence policy substitution, change during signing, governed provider-key rotation, durable-floor restart, and replay rollback. Focused Rust execution remains pending while the shared Cargo lane is occupied. | Resolve `V1-BLOCK-POTR-DUAL-SIGNER-01`: run focused/workspace validation, inject separately administered gateway Ed25519 and provider ML-DSA-65 HSM/KMS adapters into the state/admission-registry-bound reader path, then exercise independent rotation/outage/replay/crash recovery and prove four-peer exactly-once receipt/repair behavior. | open |
| `reference_sdk_release` | 6 | `docs/source/sorafs_reference_sdk_plan.md`; `sorafs-validate`, Rust reference core, ABI-21 C/Node/Python/JNI/Swift/C# wrappers, canonical fixture-bundle and governance-log-node validators plus Governance DAG block/head validation across JavaScript/TypeScript, Python, Swift, Kotlin/JVM, mirrored Java Android, and C#, and the release packager. | The test-only Ed25519-signed, schema-closed `reference_sdk_validation_inventory_v1.json` binds 82 payload artifacts, 30 exact `ValidationOutcomeV1` files, and 38 negative payload vectors across appeal finance, routing/provider admission, orderbook, PDP, PoR, PoTR, repair, Governance DAG, and moderation. The typed generator and decoder tests freeze the two-field `CancelAssetLock` V1 hard cut and reject legacy missing-field, zero-at-execution, noncanonical-quantity, and trailing-byte vectors. Its ten exact profiles comprise nine fixture-bundle outcomes plus the dedicated moderation governance-log-node outcome. The offline checker rejects tamper, missing/extra/duplicate/traversal, noncanonical/nonfinite JSON, symlink/hardlink, and parent-swap attacks; deterministic regeneration and source-level native-wrapper/byte-parity tests are checked in for the six SDK families. Native-dependent tests remain capability-gated and a skipped run is not counted as release validation. | Run the Rust regeneration test, rebuild every checked-in and published native artifact at the current ABI, and execute all six SDK exact-parity suites on supported runtimes without capability skips. Then complete mandatory platform binaries, clean-consumer packages, SBOM/provenance, published canaries, and genuine reference-deployment evidence. | open |
| `repair` | 8 | `docs/source/sorafs_repair_plan.md`; native repair records/ISIs/queries/events, Torii caller-signed one-instruction command ingress and finalized query routes, `crates/sorafs_node/src/repair_transaction_forwarder.rs`, the finalized native lease storage executor, bounded full-projection GC/reconciliation, and deletion of the local manager/store/checkpoint/API authority. | Native lifecycle/lease/action/appeal atomicity, exact-chain transaction signing and finality reconciliation, route-specific `202` command and `200` query responses, stale cursor/owner/generation/expiry rejection, malformed finalized-task and unsafe chunk-path rejection, restart deduplication, replay/rate-limit/event, checker/runner, binding, and aggregate tests. | Prove cross-peer exactly-once execution and one terminal outcome. | open |
| `reputation` | 8 | `docs/source/sorafs_reputation_plan.md`; deterministic reputation/reference/governance foundations; native governed journal policy/history, one global sequence, event/source indexes, typed committed events, fixed-view query, PoR/token append instructions, atomic capacity-dispute `Opened`/`Resolved` integration, and generic signed Torii transaction/query transport; plus the finalized-identity-keyed, single-flight, byte-identical SFM-1 authority join/cache in `sorafs_orchestrator`. The multi-feed finalized projector is exported from `sorafs_node`, consumes the existing proof/journal/repair/orderbook/reserve projections, exposes five restart-safe physical feed cursors, and persists canonical crash reconciliation plus a bounded idempotent unsigned-material retry/dead-letter/ack outbox. Strict `iroha_config` pins the release window, weights, bounds, checkpoint roots, adapter handles, and DAG publisher identity; `irohad` constructs the queue-backed journal submitter, requires an injected immutable historical finalized query plus external threshold-signer and Governance DAG clients, fails startup on missing/null/substituted dependencies, supervises reconciliation and shutdown with freshness deadlines, and exports payload-free status/metrics. Authenticated DAG readback gates the committed Torii projection and is reverified with the signed snapshot on restart. The publication checkpoint retains an immutable authenticated snapshot/readback suffix capped at 1,024 entries and its byte ceiling; snapshot-id reads return the exact retained snapshot, while unknown or evicted ids return `404`. Acknowledgement requires the full public trust-policy digest, quorum, signature, revocation, freshness, future-skew, snapshot, scoring-evidence, and signing-digest verification. The local Torii reputation POST, route catalog/OpenAPI operation, and CLI publication command are removed; rollout collection requires reviewed external publication evidence. | Journal policy predecessor/rotation, exact permission and recorder authority, source/provider/policy/block-time binding, global/source continuity, replay, forged/orphan state, bounded fixed-view pagination, and atomic dispute lifecycle tests; projector cursor gap/fork/reorder/equivocation, crash-stage recovery, checkpoint corruption, replica parity, outbox retry/restart/dead-letter/idempotency, substituted material, forged/revoked/duplicate/insufficient-quorum/stale/future signing result, restart DAG/snapshot forgery, liveness timeout/freshness, stream retention-gap, exact historical snapshot lookup, bounded eviction/unknown-id rejection, and payload-free failure tests; config missing/null/substituted adapter and checkpoint-restart tests; snapshot/consumer determinism, join concurrency/stale/fork/bounds/replica parity, route/CLI publication hard-cut, transport/checker/runner, and aggregate tests. Focused Rust validation remains pending while the shared Cargo lane is occupied. | Resolve `V1-BLOCK-REPUTATION-RUNTIME-01`: run focused/workspace validation; deploy genuine immutable historical-query, external threshold-signing, and authenticated DAG publication/readback/head-inclusion adapters; wire the PoR/token owners to the durable callbacks; add dedicated authenticated committed-event SDK projections; and prove multi-replica byte parity, signer rotation/revocation, retry/failover, recovery, and four-peer consumption. | open |
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

### V1-BLOCK-GATEWAY-CONTROLLER-RUNTIME-01 — independently administered gateway rollout

The canonical predecessor-bound controller, bounded HTTPS feed transport,
durable single-writer lease and revisioned checkpoint, atomic promotion and
last-known-good rollback, authenticated control API, serving enforcement, and
bounded telemetry are present. Those local contracts do not prove that two
independent regional administrations are consuming the same threshold-approved
catalog or that the reference deployment's appeal, legal-hold, and baseline
producers are live.

This blocker closes only when separately administered gateway launchers bind
runtime-only feed credentials and pinned trust, use independently reviewed
ACME/TLS and threshold-signing adapters, consume finalized appeal and hold
state, and pass catalog split-brain, predecessor, DNS rebinding, redirect,
decompression, timeout, lease/fencing, restart, failover, rollback, and
payload-free audit tests. The two gateways must then acknowledge the same fresh
catalog digest under distinct region and administrator identities, and genuine
serving probes must observe the exact canonical `451` denial contract. A dry
run, loopback adapter, self-signed inventory, or two processes under one
administrator cannot satisfy the lane.

### V1-BLOCK-PROVIDER-INGEST-RUNTIME-01 — authenticated source and signer deployment

The opt-in standard-daemon provider-ingest worker is present. It scans one
immutable bounded finalized replication-order snapshot, retains a monotonic
finalized high-water, acquires bounded durable source claims, verifies and
ingests provider content, constructs the exact fee-quoted completion
transaction, submits through the queue, and reconciles committed completion or
cancellation. Its process-lifetime no-follow writer lock, retention-only
terminal pruning, retry/dead-letter path, crash recovery, identity-pinned
runtime seams, blocking-work isolation, payload-free status, and worker-liveness
readiness fail closed. Storage-only nodes do not open the completion outbox
unless this worker is explicitly configured.

The repository does not yet provide the production authenticated
multi-provider source transport or governance-aware HSM/KMS completion-signer
resolver. The native ledger adapter builds its bounded immutable page set by
scanning all replication orders; a long-lived deployment therefore still needs
a provider-indexed committed query or governed terminal-history filter/archive.
The daemon now checks current committed provider ownership before signer
resolution and on both sides of signing, while every newer finalized assignment
snapshot invalidates retained `Signing`, `Signed`, `Ambiguous`, or `Submitted`
material when the owner changes or is removed. The durable signing context now
also binds the exact governed signer-policy identity, monotonic revision, and
digest. Before observation or resubmission, a newer finalized policy lookup
invalidates retained material on same-owner rotation or revocation, including
after restart. A durable policy floor rejects revision rollback,
same-revision digest equivocation, identity substitution, and post-revocation
reuse without a strict successor. The deployment resolver must supply that
exact binding and enforce key validity atomically with signing.

Shutdown remains bounded only when the deployment source reader honors the
required deadline inside each underlying read; the wrapper cannot interrupt an
inner `Read::read` that blocks indefinitely. Readiness probes fail closed at
their configured timeout and stop the supervised worker, but a timed-out
`spawn_blocking` task cannot be force-cancelled and may linger until its
provider call returns. This blocker closes only when the deployment-owned
adapters, indexed query/archive, and durable signer-policy binding are
implemented; focused/workspace and adversarial
outage/rotation/restart/capacity/deadline tests pass; and the reviewed
four-validator deployment proves one completion under simultaneous cross-peer
submission and recovery. A null/test adapter, unauthenticated or file-backed
source, local software/file/environment signing key, fabricated assignment
page, unbounded scan, or queue acceptance without committed reconciliation
cannot satisfy readiness.

### V1-BLOCK-REPUTATION-RUNTIME-01 — committed journal delivery and deployment adapters

The committed multi-feed projector and publication reconciler now have strict
non-secret configuration, runtime-only adapter injection, supervised
startup/shutdown, fail-closed monotonic freshness, payload-free status, and
bounded metrics. `QueuedReputationJournalTransactionSubmitterV1` signs and
enqueues typed PoR and counted stream-token append transactions, and the
committed projection is exposed to Torii only after authenticated DAG readback
and a fresh successful reconciliation. Persisted signed snapshots and compact
signed DAG readback material are reverified on restart. The current-head
`State` query adapter was removed because it cannot provide immutable
historical exact-anchor pages; enabling the runtime without an injected exact
historical query, external threshold signer, or authenticated Governance DAG
publication/readback adapter fails startup.

This blocker closes only when focused/workspace Rust validation passes,
deployment-owned immutable historical-query,
`ReputationThresholdSignerClientV1`, and
`ReputationGovernanceDagClientV1` adapters are injected; the PoR terminal and
stream-token owners invoke the durable journal callbacks; current DAG
head-chain inclusion is authenticated; the bounded exact retained-history
snapshot-id contract remains intact; identity/rotation/outage/restart/fork/
retry negatives pass; and the reviewed four-validator deployment proves one
committed outcome and recovery. A live current-head view, local snapshot
database, null adapter, file/environment credential, fabricated ledger page,
pre-finality submit receipt, or unsigned DAG acknowledgement cannot satisfy
readiness.

### V1-BLOCK-MODERATION-VIEWER-RUNTIME-01 — fenced orchestration and evidence authority

The native moderation ledger, fixed-view queries, bounded typed events,
finalized-chain orchestrator, exact transaction outbox, evidence authorization,
WebAuthn/grant flow, authenticated range decryption, signed hash-chained
receipts, an Ed25519-signed canonical checkpoint-digest/receipt-count/chain-head
anchor, exact checkpoint- and predecessor-bound receipt projection, retention,
legal holds, and erasure WAL are present. Audit requests require the exact
checkpoint digest and explicit digest-bound page limit, accept one raw query
ordering, return `409` on checkpoint change, and reuse the retained signature
without signer calls. Moderation GETs read only a fresh worker-owned
finalized projection. Maintenance is supervised with deadline, non-overlap,
monotonic-cursor, dead-letter, freshness, and liveness fencing. The signed
receipt checkpoint/projection is the sole evidence-access audit authority; the
old aggregate-audit POSTs are authenticated/authorized `410 Gone` tombstones,
and their scheduler and local registry calls are removed. Missing or invalid
runtime dependencies fail through typed payload-free startup errors. These
source boundaries do not yet make the stock daemon a reference-production
service. A valid anchor proves integrity and signer identity, but a first-time
consumer still needs an independently authenticated monotonic public head to
reject an older validly signed anchor.

This blocker closes only when every enabled-service dependency is constructed
by the reference runtime from runtime-only HSM/KMS/WebAuthn and authenticated
downstream providers; a durable notification-delivery worker and the
settlement/publication adapters reconcile exact operation bytes; the signed
receipt projection is connected to a transparency producer that anchors its
monotonic checkpoint head for first-contact freshness; semantic operation IDs
are fenced across replicas; viewer and orchestrator checkpoints have
predecessor-bound monotonic CAS heads and single-writer fencing; and
terminal receipts, operations, dead letters, holds, and erasure tombstones have
a signed replay-safe compaction/archive path. Same-height finalized hash
substitution, rollback, hung-provider, ambiguous delivery, capacity,
WebAuthn-replay, IDOR, legal-hold race, restart, and multi-instance takeover
negatives must pass before four-validator deployment evidence can close the
lane.

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

### V1-BLOCK-APPEAL-SETTLEMENT-DEPLOYMENT-01 — reference HSM and four-peer proof

The source boundary is now production-shaped: appeal-finance configuration
contains only bounded opaque signer handles, exact public keys, authorities,
and finalized-height activation/revocation windows; no private key is accepted.
Authenticated mutation routes persist native `OpenAssetLock`,
`DrawdownAssetLock`, or `CancelAssetLock` work before signing. A supervised
worker durably tracks its finalized cursor, exact signed bytes, retry state, and
dead letters; verifies the runtime provider identity and returned transaction;
reconciles exact transaction results plus committed escrow state; and publishes
receipts only after finality. Native drawdowns and cancellations compare the
exact committed remaining amount observed by the worker before moving custody,
preventing concurrent peers from applying the same stale partition twice or
racing a refund against a drawdown. Refund follow-up is durable before a
completed drawdown is removed, and bounded completion capacity never evicts an
exactly-once tombstone. Standard `irohad` forwards only an explicitly injected
runtime registry and never manufactures a signer from the node key.

This implementation has not yet been exercised by the focused Rust/workspace
suite or the reviewed reference deployment. The blocker closes only when the
reference launcher injects an independently administered PKCS#11/HSM/KMS
provider through the opaque-handle registry and passes wrong-key/authority/
signature, provider outage, activation/rotation/revocation, restart, stale-fork,
duplicate, retry-exhaustion, crash-boundary, and four-validator exactly-once
lock/refund/disbursement tests. The deployment must also validate dashboard and
alert routing and produce signed payload-free appeal-finance evidence. A test
signer, raw file/config/environment key, software fallback, request-only
one-step execution, or pre-finality local receipt cannot satisfy the lane.

### V1-BLOCK-POTR-DUAL-SIGNER-01 — independently administered receipt signers

PoTR receipt encoding, persistence, validation, native repair convergence, and
the role-separated source boundary are present. Torii accepts only distinct
injected gateway Ed25519 and provider ML-DSA-65 signer objects with distinct
stable administrative identities. It also requires a separate stable-identity
live admission reader and a non-zero exact baseline policy anchor. The reader
is queried before signing and again after both signatures; unavailable,
revoked, stale, identity-drifting, same-sequence-substituted, or mid-signature
changed policy fails closed. The tracker persists the exact policy
identity/digest/sequence, finalized height/hash, provider, and admission
envelope before any proof-outcome or repair handoff, then restores that binding
as the next query's monotonic floor. Torii no longer treats its startup
admission-registry snapshot alone as receipt authorization.

The authenticated reader path is concrete:
`PotrStateFinalizedPolicySourceV1` reads Torii's authoritative state and
`PotrFinalizedAdmissionReaderV1` combines that policy with the council-verified
admission registry. The generic launcher intentionally supplies no
gateway/provider signer roles. This blocker closes only after
focused/workspace Rust tests pass, the reference deployment injects
independently administered PKCS#11/HSM/KMS gateway and provider adapters, and
passes wrong-role, cross-provider, revoked/stale/substituted policy, replay,
partial-signature, reader/signer outage, independent rotation, restart,
four-peer recovery, and receipt/repair exactly-once tests. A startup registry
alone, self-advertised key, fabricated policy binding, file or environment key,
or signer-derived admission reader cannot satisfy the lane.

### V1-BLOCK-GOVERNANCE-DAG-RUNTIME-01 — sealed runtime identity and checkpoint

The Governance DAG service, authenticated bounded JSON backend, leader/failover
mechanics, Kubo/IPFS/IPNS publication, checkpoint recovery, health, and metrics
exist. The source boundary now removes every signing-key, checkpoint-key, and
bearer-token path: the embedded publisher requires an injected runtime signer
whose opaque handle, peer identity, and canonical non-weak Ed25519 public key
match configuration, while the supervised service requires injected
rotation-aware endpoint authenticators and a sealed monotonic CAS checkpoint
store. Missing, mismatched, drifting, malformed, replayed, rolled-back, or
unavailable providers fail closed with provider diagnostics redacted. The
generic packaged launcher intentionally supplies no secret provider.

This blocker remains open only for deployment-owned provider adapters and
reference-production evidence. It closes after the supported bundles supervise
two instances with HSM signing, governed Kubo/head authentication, sealed
checkpoint custody, and successful CAS/failover, credential rotation, rollback,
checkpoint corruption, signer/authenticator outage, public-mirror, and
disaster-recovery rehearsals on the reviewed deployment.

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
