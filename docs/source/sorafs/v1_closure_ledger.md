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
| 2 | SF-1 | Canonical manifest, registry, provider advert, alias, storage-ingest, and retrieval contract. | Manifest/registry data model, Torii admission, fixture and SDK guards, plus the opt-in supervised finalized-ledger provider-ingest worker with a bounded immutable assignment snapshot, exact fee-quoted transaction construction, committed reconciliation, and fail-closed liveness. Its external sealed monotonic checkpoint authority is bound by stable handle/revision/public-policy digest, stores canonical predecessor/digest/bounded-checkpoint records, performs authoritative pre/post readback, and treats the local file as a revalidated cache only. The source-level daemon-owned Kura-authenticated provider-indexed archive implementation supplies exact-anchor pages, restart reconciliation, typed retention-floor errors, and a separate canonical proposal/approval protocol that forbids checkpoint installation or prefix cleanup before exact sealed-CAS readback. Manual startup rejects checkpoint candidates; authority startup recovers only the exact approved fence/checkpoint lineage. This local implementation does not establish production-adapter availability or deployment qualification. | Resolve `V1-BLOCK-PROVIDER-INGEST-RUNTIME-01` by wiring and qualifying the real authenticated source transport, governance-aware HSM/KMS signer resolver, and deployment-owned sealed-CAS backend against the implemented archive-retention authority contract; then collect final pin/alias/provider/ingest/retrieval receipts from the reviewed deployment. | open |
| 3 | SF-2 | PDP provider protocol and proof validation. | Canonical PDP schemas, the durable provider protocol, authenticated Torii challenge/next-work/proof/status/terminal-export family, fail-closed exact-chain native repair-transaction handoff, finalized reconciliation, and storage execution gated by the exact finalized lease are implemented locally. The competing local repair projection is deleted. | Authenticated deployed provider transport, multi-provider proof replay, cross-peer exactly-once repair, and observability. | open |
| 4 | SF-2c | PoR/PoTR challenge, receipt, and repair convergence. | PoR/PoTR schemas, coordinators, reference validation, final dual-signed PoTR receipt persistence with an exact finalized admission-policy binding, 32-byte native repair task identities, fail-closed exact-chain native handoff, finalized-lease-gated storage execution, exactly-once latency-repair identity, PoR latency/VRF/seed metrics, `PotrStateFinalizedPolicySourceV1`, `PotrFinalizedAdmissionReaderV1`, the strict independent `[sorafs.por.potr_runtime]` public role binding, and the local repair-authority removal are implemented. | Live provider/auditor runs through the production finalized reader, governed provider-key rotation, dual-signed durable receipts, cross-peer exactly-once repair, and archive evidence. | open |
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
| `ai_prescreen` | 8 | `docs/source/sorafs_ai_prescreen_plan.md`; config-pinned canonical screening authority, signed-result/exact-committee Torii admission, chunked ChaCha20-Poly1305 quarantine envelopes, runtime PKCS#11/KMS wrapper boundary, stock-daemon authenticated local-broker client/server protocol, and `scripts/check_sorafs_ai_prescreen_rollout_evidence.py`. | Authority digest/canonical/path bounds, signer/quorum/policy/manifest/binding/freshness/replay negatives; AEAD wrong key/tag/AAD/chunk/range/recovery/rewrap and payload-redaction tests; broker catalog/key-handle/payload-bound/substitution/drift tests; checker, runner, builder, and aggregate binding. The newest broker tests remain pending focused Cargo execution. | Finish `V1-BLOCK-AI-QUARANTINE-KMS-01` by supplying and qualifying a genuine deployment-owned PKCS#11/managed-KMS broker backend, then deploy the trained committee workflow and KMS-backed quarantine service, exercise outage/recovery/rotation, publish DAG/transparency records, and collect end-to-end evidence. | open |
| `appeal_finance` | 10 | `docs/source/sorafs_appeal_pricing_plan.md`; deterministic pricing and plan-only settlement APIs, native escrow projections, bounded durable semantic transaction outbox/cursor/dead-letter state, supervised finalized-chain reconciliation, runtime-only opaque Ed25519 signer bindings with activation/revocation, strict returned-transaction verification, and post-finality Governance DAG receipts. `iroha_config` accepts no appeal-finance private key, and standard `irohad` does not derive a provider from its node key. | Quote/deposit/settlement/config binding and aggregate tests; source-level adversarial tests cover signer substitution, wrong authority/signature, rotation/revocation boundaries, duplicate/substitution replay, crash recovery, retry exhaustion, stale fork/cursor, reconciliation, and poisoned checkpoints. Focused Rust execution remains pending while the shared build lane is occupied. | Resolve `V1-BLOCK-APPEAL-SETTLEMENT-DEPLOYMENT-01`: run focused/workspace validation, inject a genuinely administered PKCS#11/HSM/KMS provider into the reference launcher, exercise outage/rotation/restart/fork and four-peer exactly-once lock/refund/disbursement, validate alert routing, and collect signed payload-free deployment evidence. | open |
| `gateway_compliance` | 10 | `docs/source/sorafs_gateway_compliance_plan.md`; signed predecessor-bound controller core, bounded runtime feed transport, process-lifetime stable-file lease, revisioned exact-byte compare-and-swap checkpointing, same-directory atomic replace with file/parent sync and post-write fencing, LKG promotion/rollback, scoped toggle/appeal/hold precedence, canonical region/gateway identities, six authenticated Torii control routes, `AppState`-retained controller/transport, live manifest/CID/provider enforcement, canonical HTTP 451 `gateway_compliance_denied` serving contract and fail-closed 503 unavailable contract, unsigned-bootstrap mutual exclusion, ACME fail-closed source guard, bounded `torii_sorafs_gateway_compliance_*` telemetry with dashboard/alerts, and compliance evidence gate. The Torii/config local store, routes, CLI/xtask mutation commands, CI helper, sample bundle, legacy `sorafs_car` denial/proof wire, and readiness/self-cert `denylist_*` inventories are removed. The gate now binds canonical catalog promotion/predecessor/precedence evidence, exact observed `451` probes, and acknowledgements from distinct gateway, region, and administrator identities. | Gateway controller signature/predecessor/quorum/precedence/persistence, dual-controller lease, restart, stale-CAS, pre-persist conflict, crash-before/after-replace, revision rollback, truncation, symlink/hardlink, and unsafe-permission negatives; SSRF/DNS-rebinding/pin/redirect/decompression negatives; canonical-body/auth/role/error mapping, serving-scope/451/503/fail-closed/bootstrap-exclusion tests; bounded-label telemetry and dashboard/alert static contracts; TLS runtime-contract static guard; feed, policy, catalog, payload-safety, binding, runner, aggregate, legacy-field/code rejection, distinct-administrator, and observed-probe negatives. | Resolve `V1-BLOCK-GATEWAY-CONTROLLER-RUNTIME-01`: independently audited standard-daemon feed and ACME adapters, finalized appeal/hold catalog producers, external threshold signing, two-gateway deployment/promotion/rollback, and genuine probes. | open |
| `gateway_load` | 5 | `docs/source/sorafs_gateway_load_tests.md`; deterministic 1,000-stream conformance harness. | Local conformance, staged-load schema, SLO, transport-scope, and aggregate tests. | Live cold/warm/mixed multi-provider runs, corruption/failover/flood pressure, and 24-hour soak. HTTP/3 is not applicable to V1. | evidence-pending |
| `governance_dag` | 8 | `docs/source/sorafs_governance_dag_plan.md`; `sorafs_governance_dag`, bounded authenticated JSON mirror, IPFS/IPNS publication, checkpoint, health, and metrics. The embedded publisher accepts only an injected runtime signer and an independently config-pinned sealed-CAS store. Its producer-specific sealed genesis checkpoint and write-ahead intent bind the canonical root, exact signer/store qualifications, predecessor, block, head, index, and resulting digests; block/head/full-index bytes are durably staged and read back while the sealed producer intent carries only exact descriptors under a 64 KiB ceiling. Restart reconciliation precedes a bounded canonical full-root audit. Explicit provider and signing-key rotation uses a canonical outgoing/incoming dual-signed key-transition envelope binding the exact sealed predecessor, current head/index, both publisher identities and Ed25519 keys, both signer/store public qualifications, monotonic authority-segment revisions, and transition/archive heads. Recovery reconstructs the bounded archived-plus-live lineage, validates each retained block under its active authority segment, and binds the head to the tip segment. Signed qualification archives are bounded to 64 transitions and 64 linked archives; immutable archive and sealed-CAS readback precede prune, and restart replays staged or committed compaction idempotently. The service exposes a supervised library launcher requiring rotation-aware IPFS/head authenticators and separate sealed state; all former signing-key, checkpoint-key, and bearer-token path settings are removed and the provider-free packaged launcher fails closed. On Linux and macOS, stock `irohad` resolves the Governance DAG signer, sealed-CAS, and IPFS/head request-authenticator roles through the shared fixed, canonical, session/request-bound local broker. The authenticator protocol signs one canonical descriptor/envelope under configuration-pinned strong Ed25519 keys and enforces bounded body, lifetime, skew, replay, and pre/post provider qualification; unsupported platforms fail closed. | Source tests cover missing/mismatched/drifting providers, malformed/weak keys, bad signatures, provider-error redaction, sealed genesis and intent recovery, staged-artifact substitution, oversized producer intent, ambiguous checkpoint CAS, explicit distinct-key transition and restart recovery, authority-segment readback, archive-before-CAS and post-CAS/pre-prune recovery, idempotent compaction, transition/envelope/archive tamper, replay, segment-revision rollback, fork, duplicate, truncation, trailing bytes, qualification substitution, unsafe temp paths, root/signature/CID/lineage/index/source tamper, sealed-state CAS/rollback/replay/deletion, legacy secret-path rejection, canonical request and envelope tamper/replay/freshness/key/body/alias rejection, and broker framing/catalog/session/request/qualification/substitution negatives. Focused Rust execution of the latest producer and broker hardening remains pending; rollout checker/runner, mock-IPFS, and opt-in Kubo lane remain. | Finish `V1-BLOCK-GOVERNANCE-DAG-RUNTIME-01`: package a deployment-owned broker executable and inject genuine HSM/sealed-store/authenticated Kubo/head backends into the in-tree protocol; exercise the implemented publisher-key/DAG-segment transition and finish bounded block-prefix retention; then supervise two instances with governed credentials, CAS/failover, public mirror, alert-routing, rollback, and recovery evidence. RocksDB/IPLD is conditional on the SF-12 capacity gate. | open |
| `hedging_billing` | 8 | `docs/source/sorafs_hedging_plan.md`; canonical feed/pricing/billing payloads and bridge; durable signed-feed high-water state; bounded finalized event/period-close ingest; deterministic accrual, governed statement, aggregate exposure, and hedge-intent projection; durable signing/publication/acknowledgement/reconciliation/dead-letter state; sealed epoch witnesses; a supervised `irohad` worker with strict non-secret `iroha_config`, opaque identity-pinned finalized-query/journal-verifier/HSM-signer/publisher/acknowledgement/witness adapter seams, payload-free health/alert metrics, and no automatic-execution loop; plus seven canonical-account-authenticated, bounded, private/no-store billing, reconciliation, exposure, and intent routes backed only by the committed projection. The Rust client and standard CLI now expose those seven read/acknowledgement routes with strict lowercase identifiers, bounded pages and proofs, redacted authentication material, secure direct-file proof reads on Unix/Windows, and no-clobber Norito statement output. Every intent disables automatic execution, and the separately authorized submission helper rejects automatic adapters. | Fixture, price/cycle/policy binding, route/catalog/OpenAPI authentication and schema contracts, runner, and aggregate gates are present. Focused client validation is green: two signed-request/strict-input tests, eighteen CLI parse/input/proof tests across the three binary targets, and twelve exact-byte/no-clobber/symlink statement tests across those targets. Focused Rust verification of the service/runtime remains pending. | Deploy live external feeds and genuine finalized-query, journal-verifier, HSM/KMS signer, immutable publisher, acknowledgement-authority, sealed-witness, and any manually governed venue adapters; deploy and exercise the shipped API and CLI; validate scrapes and alert routing; then collect staged billing/reconciliation and rollout evidence. Automatic execution remains disabled. | open |
| `moderation_panel` | 12 | `docs/source/sorafs_moderation_panel_plan.md`; native bounded moderation ISIs/queries/events, finalized-chain orchestrator, exact retained transaction bytes, ambiguous-ingress reconciliation, exact-once terminal handoff contracts, and a supervised payload-free panel-notification worker that checkpoints claims before calling an independently qualified durable boundary and persists stable receipts or bounded dead letters. `iroha_config`, standard `irohad`, and Torii hard-bind that boundary by a non-secret production handle, revision, and policy digest. Reads use a fresh worker-owned projection with deadline/non-overlap/freshness supervision, while the evidence viewer retains its signed predecessor-bound checkpoint under an externally injected qualified CAS authority and exposes the exact predecessor-bound receipt projection as the sole audit authority. Its local file is only a verified cache. The viewer now also pins an immutable archive's handle/revision/policy, namespace id, and Ed25519 key through daemon slot 47. A shutdown-aware bounded worker uses signed checkpoint/archive fences and stable operation ids, requires exact canonical archive readback plus a valid archive receipt signature before prune, and persists the monotonic signed archive head through the authoritative CAS. The stock broker protocol covers all six viewer slots with exact metadata, bounded canonical operations, typed mutation ambiguity, and authenticated checkpoint/archive readback. The retired aggregate-audit POSTs are authenticated/authorized `410 Gone` tombstones and have no scheduler or local registry mutation. | Contiguous committed-event replay, exact sortition/selection authority, commit/reveal, restart reconciliation, duplicate cross-peer submission, no-show/failover, policy/case/roster/tally bindings, hung/stale/dead-letter/cursor-equivalence supervision, notification retry/idempotent replay/qualification-drift/receipt-checkpoint safety, viewer authorization/WebAuthn/grant/range/receipt/projection-cursor/hold/erasure/CAS safety, archive-before-prune/replay/restart/bounded-tick safety, archive signature/trailing/fork/rollback/skipped-generation/provider-substitution/drift negatives, and broker slot/shape/bounds/ambiguity tests. Focused Rust validation of the latest hardening remains pending. | Resolve `V1-BLOCK-MODERATION-VIEWER-RUNTIME-01`: package the stock broker service and supply genuine deployment-owned notification, settlement, publication, HSM/KMS/WebAuthn, linearizable sealed-CAS checkpoint-store, immutable object-lock archive, and signed receipt-to-transparency providers; prove operation-ID and checkpoint/archive fencing, exact replay, and recovery under multiple replicas; then collect four-peer case, failover, recovery, and payload-free promotion evidence. | open |
| `orderbook` | 9 | `docs/source/sorafs_orderbook_plan.md`; canonical payloads in `crates/sorafs_manifest/src/orderbook.rs`, native policy/state in `crates/iroha_data_model/src/sorafs/orderbook.rs`, atomic execution in `crates/iroha_core/src/smartcontracts/isi/sorafs_orderbook.rs`, finalized Torii projections in `crates/iroha_torii/src/sorafs/api.rs`, durable native transaction forwarding/reconciliation, and the supervised finalized-ledger provider-ingest completion worker. The competing `sorafs_node` book, checkpoint, config, mutation/event surface, settlement publisher, and obsolete runtime-snapshot wire/SDK selectors are deleted. | Native admission sequence/revision/order/fill/trade/channel/escrow/expiry/refund model tests, exact governed matcher/settlement authority negatives, finalized query/cursor tests, provider-ingest retry/restart/tombstone/finality/identity negatives, retired-selector rejection, reference validation, checker/runner, policy/contract bindings, and aggregate tests. | Complete source/SDK validation, resolve `V1-BLOCK-PROVIDER-INGEST-RUNTIME-01`, then collect four-peer simultaneous-submission, provider-ingest restart/rotation, expiry/refund race, single-settlement, and recovery evidence from the reviewed deployment. | open |
| `pdp` | 6 | `docs/source/sorafs_pdp_plan.md`; `crates/sorafs_node/src/pdp_provider.rs`, PDP manifest modules, authenticated Torii provider routes, challenge-bound proof streaming, exact-chain durable native repair transaction handoff, finalized-lease-gated execution, and deletion of the local repair projection. | Protocol/reference, malformed/canonical/replay/admission/restart, native-handoff failure/retry/reconciliation, stale/wrong lease and restart-deduplication tests, checker/runner negatives, and a static guard that requires the five-route V1 API while rejecting reserved placeholder routes. | Exercise the authenticated protocol across multiple deployed providers and collect cross-peer repair plus live metrics/archive evidence. | open |
| `pop_credentials` | 9 | `docs/source/sorafs_pop_credentials_plan.md`; native PoP registry, `crates/sorafs_node/src/pop_credentials.rs`, and the canonical authenticated 14-route Torii V1 service. | Durable encrypted enrollment/wallet, dual control, HSM/KMS interfaces, outbox/reconciliation, canonical/depth/allocation/auth/time rollback negatives, privacy, runner, and aggregate tests. | Resolve `V1-BLOCK-POP-RUNTIME-01`: package and wire the governed external-runtime/sidecar provider bundle into standard `irohad`, then exercise HSM issuance, reconciliation/revocation, wallet custody, local proofs, verifier replay defense, restart, and rotation without ledger/log secrets or PII. | open |
| `por` | 6 | `docs/source/sorafs_por_plan.md`; PoR scheduler/randomness/governance foundations, request-bound sampling, emitted latency/VRF/seed metrics, exact-chain durable native repair handoff, finalized-lease-gated execution, and deletion of the local repair projection. Finalized verdicts retain exact sequence/digest-bound reputation work until durable native admission and acknowledgement. The optional hard-cut replay archive pins a production handle, immutable archive identity, revision, policy digest, and strong Ed25519 verification key; signed predecessor-linked receipts bind the canonical record and reputation work. Standard challenge/verdict paths use the qualified provider automatically, and a supervised worker performs bounded reputation-first reconciliation and authenticated compaction. Local replay state is pruned only after authenticated `current_head` readback equals the final receipt and the provider binding remains unchanged. | VRF/seed/request replay, proof/reference, native 32-byte task identity, handoff retry/signature/checkpoint corruption, stale/wrong lease, restart deduplication, metric-label bounds, archive append/crash-retry/readback/signed-chain/tamper and signed-but-stale-head tests, config missing/disabled/partial/test-marked/bounds/secret-field negatives, provider missing/unrequested/substituted/stale/drift qualification tests, launcher source contracts, checker/runner, and aggregate tests. The newest config/launcher tests remain pending focused Cargo execution. | Supply and qualify a genuine deployment-owned immutable archive plus external Ed25519 HSM signer for slot 46, then run live randomness with provider/auditor scheduling and replay, prove cross-peer exactly-once reputation/repair/archive recovery, archive the reporting output, and obtain governed approval. | open |
| `potr` | 6 | `docs/source/sorafs_potr_plan.md`; `crates/sorafs_node/src/potr.rs`, proof-stream/governance schemas, fail-closed exact-chain durable native repair handoff, finalized-lease-gated execution, deletion of the local repair projection, and Torii's injected `PotrRuntimeSignersV1` boundary. The stream-token issuer no longer owns or derives a provider ML-DSA key. Distinct gateway/provider runtime objects and administrative identities are mandatory. The strict optional `[sorafs.por.potr_runtime]` binding independently pins both signers, their qualifications, the gateway key, distinct reader/source/resolver identities, and the complete baseline finalized admission anchor; enabled startup requires exact equality with injected roles. Torii constructs `PotrFinalizedAdmissionReaderV1` from `PotrStateFinalizedPolicySourceV1` and the council-verified admission registry, resolves the exact live policy before every receipt, and rechecks it after both signatures; the startup registry alone is not authorization. The tracker atomically persists provider, policy identity/digest/sequence, finalized height/hash, and exact admission-envelope digest, and exposes that retained anchor as the floor for the next read. | Final receipt/reference, signature-shape, persistence/restart, native repair idempotency, stale/wrong lease, restart deduplication, PQ roster, reputation binding, checker/runner, and aggregate tests; source tests cover missing/unconfigured/substituted runtime roles, partial/disabled/test-marked/shared config, identity collisions, shared/drifting signer and reader identities, wrong role/provider/key/algorithm, inactive or untrusted admission, partial invalid output, reader/signer outage, revocation, stale and same-sequence policy substitution, change during signing, governed provider-key rotation, durable-floor restart, and replay rollback. Focused Rust execution remains pending while the shared Cargo lane is occupied. | Resolve `V1-BLOCK-POTR-DUAL-SIGNER-01`: run focused/workspace validation, inject separately administered gateway Ed25519 and provider ML-DSA-65 HSM/KMS adapters into the state/admission-registry-bound reader path, then exercise independent rotation/outage/replay/crash recovery and prove four-peer exactly-once receipt/repair behavior. | open |
| `reference_sdk_release` | 6 | `docs/source/sorafs_reference_sdk_plan.md`; `sorafs-validate`, Rust reference core, ABI-21 C/Node/Python/JNI/Swift/C# wrappers, canonical fixture-bundle and governance-log-node validators plus Governance DAG block/head and appeal-finance cancellation validation across JavaScript/TypeScript, Python, Swift, Kotlin/JVM, mirrored Java Android, and C#, and the release packager. | The resealed test-only Ed25519-signed `reference_sdk_validation_inventory_v1.json` binds 82 payload artifacts, 32 exact `ValidationOutcomeV1` files, and 38 negative payload vectors across appeal finance, routing/provider admission, orderbook, PDP, PoR, PoTR, repair, Governance DAG, and moderation. All eight generated `CancelAssetLock` payload files are checked in, mandatory, byte-bound, and validated by the offline checker. The transparent V1 `EscrowId` hash representation and retired nested binary/JSON representations are frozen by Rust and SDK tests. The fixture checker rejects tamper, missing/extra/duplicate/traversal, noncanonical/nonfinite JSON, symlink/hardlink, and parent-swap attacks. Host C/JNI, C#, Node, and pinned-Python-3.12 native lanes now record and reverify stable artifact bytes, exact clean source identity, exact ABI 21, and required appeal-finance symbols; Apple/Swift and Android packages retain the separate per-slice source-sealed mobile artifact gate, and Android additionally requires both exact `NativeSignerBridge` JNI contract-revision probe exports. The obsolete tracked `_crypto.cpython-39-darwin.so` is removed. The mandatory Python 3.12 runner rejects every tracked package `.so`, `.so.*`, `.dylib`, `.pyd`, or `.dll`, activates its virtual environment, covers cancel-asset-lock, reference-validation, and provider-ingest suites, and rejects JUnit skips; its static workflow-file contract is green at 9/9. The separate pin-register workflow, runner, and guard are exact Python 3.12 and install only `requirements-ci.lock` with `--require-hashes --only-binary=:all:`. A fresh isolated CPython 3.12.13 venv passed 3/3 tests plus positive static and changed-version/resolver/major/workflow/lock-removal negatives. Release-required SDK tests fail rather than count an unavailable/stale bridge as passing. No clean five-target artifact rebuild or provenance record has yet been recorded, and no native release binary is tracked in the source tree. | Build every required native artifact from one clean pinned commit for Linux x86_64/aarch64, macOS x86_64/aarch64, and Windows x86_64, execute all six SDK exact-parity suites without capability skips, and publish only the authenticated results. Then complete clean-consumer packages, SBOM/provenance, published canaries, and genuine reference-deployment evidence. | open |
| `repair` | 8 | `docs/source/sorafs_repair_plan.md`; native repair records/ISIs/queries/events, Torii caller-signed one-instruction command ingress and finalized query routes, `crates/sorafs_node/src/repair_transaction_forwarder.rs`, the finalized native lease storage executor, bounded full-projection GC/reconciliation, and deletion of the local manager/store/checkpoint/API authority. | Native lifecycle/lease/action/appeal atomicity, exact-chain transaction signing and finality reconciliation, route-specific `202` command and `200` query responses, stale cursor/owner/generation/expiry rejection, malformed finalized-task and unsafe chunk-path rejection, restart deduplication, replay/rate-limit/event, checker/runner, binding, and aggregate tests. | Prove cross-peer exactly-once execution and one terminal outcome. | open |
| `reputation` | 8 | `docs/source/sorafs_reputation_plan.md`; deterministic reputation/reference/governance foundations; native governed journal policy/history, one global sequence, event/source indexes, typed committed events, fixed-view query, PoR/token append instructions, atomic capacity-dispute `Opened`/`Resolved` integration, and generic signed Torii transaction/query transport; plus the finalized-identity-keyed, single-flight, byte-identical SFM-1 authority join/cache in `sorafs_orchestrator`. The multi-feed finalized projector is exported from `sorafs_node`, consumes the existing proof/journal/repair/orderbook/reserve projections, exposes five restart-safe physical feed cursors, and persists canonical crash reconciliation plus a bounded idempotent unsigned-material retry/dead-letter/ack outbox. Strict `iroha_config` pins the release window, weights, bounds, checkpoint roots, adapter handles, and DAG publisher identity; standard `irohad` requires an externally authenticated journal-transaction submitter, immutable historical finalized query, external threshold signer, and Governance DAG client, and has no validator-key, queue-backed, or current-head fallback. Enabled startup fails on missing/null/substituted dependencies, supervises reconciliation and shutdown with freshness deadlines, and exports payload-free status/metrics. Authenticated DAG readback gates the committed Torii projection. Its canonical V1 receipt carries a pinned signed head and a contiguous inclusion suffix bounded by the manifest checkpoint window, requires the exact signed snapshot once, links successor receipts to the previously authenticated head, and persists/reverifies the head and every path block on restart. The publication checkpoint retains an immutable authenticated snapshot/readback suffix capped at 1,024 entries and its byte ceiling; snapshot-id reads return the exact retained snapshot, while unknown or evicted ids return `404`. Acknowledgement requires the full public trust-policy digest, quorum, signature, revocation, freshness, future-skew, snapshot, scoring-evidence, and signing-digest verification. The local Torii reputation POST, route catalog/OpenAPI operation, and CLI publication command are removed; rollout collection requires reviewed external publication evidence. All seven committed GET routes, strict JavaScript/TypeScript, Python, Kotlin/JVM, Java Android, Swift, and C# read clients, plus the canonical-account-signed Rust CLI and collector wiring, are implemented locally. PoR verdict ownership now retains typed terminal work and the standard launcher supervises durable exact admission/acknowledgement before replay-archive compaction. Stream-token outcomes use a hard-cut gateway-id/non-zero-sequence/request-context binding and a bounded durable per-gateway high-water mark; exact replay is idempotent while stale or substituted sequence reuse fails closed across restart. | Journal policy predecessor/rotation, exact permission and recorder authority, source/provider/policy/block-time binding, global/source continuity, replay, forged/orphan state, bounded fixed-view pagination, and atomic dispute lifecycle tests; projector cursor gap/fork/reorder/equivocation, crash-stage recovery, checkpoint corruption, replica parity, outbox retry/restart/dead-letter/idempotency, substituted material, forged/revoked/duplicate/insufficient-quorum/stale/future signing result, signed-head/path tamper, oversize/no-truncation, exact-target, rollback/fork, provider-drift, restart readback, liveness timeout/freshness, stream retention-gap, exact historical snapshot lookup, bounded eviction/unknown-id rejection, and payload-free failure tests; config missing/null/substituted adapter and checkpoint-restart tests; PoR terminal mapping, durable acknowledgement, archive/crash-retry, launcher/provider qualification tests; stream-token sequence-reuse, restart, older-finalization, multi-row policy-rotation, and bounded-head tests; snapshot/consumer determinism, join concurrency/stale/fork/bounds/replica parity, route/CLI publication hard-cut, seven-route canonical-auth/cache/stream tests, strict SDK client and signed CLI/collector tests, transport/checker/runner, and aggregate tests. Prior focused locked validation was green; the latest PoR archive/launcher and stream-token hardening is pending focused and workspace validation. | Resolve `V1-BLOCK-REPUTATION-RUNTIME-01`: validate Kura-authenticated compact historical capture/startup and the new PoR archive wiring; deploy genuine immutable historical-query, external journal submission, external threshold-signing, authenticated DAG publication/readback, and finalized-PoR replay-archive adapters matching their signed contracts; wire the stream-token owner to the durable callback; execute the complete SDK/native validation matrix; and prove multi-replica byte parity, signer rotation/revocation, retry/failover, recovery, four-peer consumption, and promotion. | open |
| `reserve_rent` | 11 | `docs/source/sorafs_reserve_rent_plan.md`; native reserve policy/provider/movement/rent/lifecycle/credit/repayment/appeal records and ISIs; exact caller-signed one-instruction Torii mutation ingress; authenticated finalized policy/provider/movement/appeal/event projections; durable native forwarding workers; finalized-event/provider telemetry with bounded labels, reconciliation readiness, and represented height. The process-local `sorafs_node` reserve runtime, checkpoint, scheduler, mutation API, obsolete routes, and telemetry authority are deleted. | Native accounting/conservation/authority/lifecycle/event/query tests; signed-envelope, route/instruction/authority/policy/revision/cursor/authentication negatives; empty-page event-resume and atomic telemetry-rebuild regressions; Prometheus rule tests; fresh scrape digest/reconciled-height evidence negatives; matrix/ledger/policy binding, checker/runner, and aggregate tests. | Complete source validation, then prove custody, fork, retry, signer/failover, projection rebuild, and peer reconciliation on the reviewed deployment. | open |
| `transparency` | 5 | `docs/source/sorafs_transparency_plan.md`; canonical fixed-population/fixed-metric public aggregates with retired exact source fields rejected; exact integer discrete-Laplace sampling; stable query/window release identity; private tagged source digests; durable hash-chained composition-budget and release ledgers; atomic source deletion/outbox persistence; finalized-head reconciliation; exact non-secret threshold-PRF/release-anchor/leader-lease config pins; production-only runtime traits; standard Node/Torii/`irohad` construction of qualification wrappers before persistence; and an external sealed-CAS leader-lease boundary whose public fencing floor is checkpointed before use and whose exact live grant is carried through release-anchor mutation, Governance DAG publication, and durable retry in `crates/sorafs_manifest/src/transparency.rs`, `crates/sorafs_node/src/transparency.rs`, `crates/sorafs_node/src/lib.rs`, Torii, config, and `irohad`. One exact `fenced_privacy_publisher_*` binding now pins both the raw `FencedTransparencyPublisherV1` writer and its independent authenticated `FencedTransparencyAuthoritativeHeadReaderV1`; Torii performs early pair qualification before global startup, while a prebuilt node must retain and live-revalidate the same pair. Enabled privacy publication also requires an explicit `governance_dag_dir`, the complete peer/signer revision/policy/public-key binding, the exact producer checkpoint-store binding, and injected runtime signer/store providers; no unsigned or unsealed directory fallback remains. The publication retry digest excludes lease, fence, and predecessor metadata, so exact retries remain stable across lease/failover changes, while authenticated exact-publication inclusion and ancestry to the current authoritative head are mandatory for `AlreadyIncluded`. The filesystem publisher records public fence metadata but is not evidence of a globally linearizable deployment adapter. | Public-schema rejection, clipping, fixed-bucket DP+k, joint-sensitivity, sampler, config pin/mode/partial/disabled negatives, missing/unrequested leader-lease startup failures, startup substitution/staleness/qualification mismatch/redaction, pre-persistence failure, fencing-floor restart restoration, per-use drift, release-chain tamper, budget, checkpoint rollback/equivocation, source-ingest response, scheduler, checker/runner, and aggregate tests. Source tests additionally cover partial/ambiguous/missing/unexpected/substituted fused roles, writer/reader qualification drift, incomplete or directory-less signer binding, stable `AlreadyIncluded` retry after lease/predecessor change, conflicting evidence, and exact inclusion/ancestry proof rejection. Focused Rust execution of the latest fused-runtime wiring remains pending. | The stock daemon registry and broker boundary is in-tree and now covers Governance DAG signer/store, moderation quarantine wrapping, and provider-ingest roles, but every transparency slot still fails closed. Extend and package it with independently administered threshold-PRF, finalized release-anchor, sealed monotonic-CAS leader-lease, fused writer/authenticated-reader, and genuine HSM Governance DAG signer/storage backends matching the exact config pins; connect every finalized producer; add public replicas/proofs/pagination/ETags and hardened explorer delivery; then capture multi-replica fencing/failover, inclusion/ancestry, differencing/budget-exhaustion, rollback, and genuine external qualification evidence. L0, L1, and L2 remain open. | open |

## Active local release blockers

### V1-BLOCK-AI-QUARANTINE-KMS-01 — standard-daemon quarantine key adapter

The signed screening boundary, config-pinned canonical authority bundle,
chunked ChaCha20-Poly1305 envelopes, per-object DEKs, authenticated range
decryption, atomic recovery, rewrap, Torii runtime dependency, and
`Iroha::start_with_runtime_deps` handoff are present. Enabling authenticated
screening without a runtime `ModerationQuarantineKeyWrapper` now fails closed at
node startup.

On Linux and macOS, stock `irohad` now resolves this slot through the fixed,
same-service-UID runtime-provider broker. The canonical handshake binds the
exact configured provider handle, revision, policy digest, and active public
`pkcs11:`/`kms:` key handle. Bounded wrap/unwrap operations carry no provider
credential or provider diagnostic, reject noncanonical handles and
malformed/oversized payloads, and requalify identity, policy, and active-key
state around every external operation. The wire and operation APIs preserve a
fixed payload-free failure class: uncertain wrap dispatch is ambiguous and is
never replayed, while read-only unwrap uncertainty is unavailable. All
secret-bearing canonical/frame/request/response buffers are overwritten on
drop. A fixed operation discriminator enforces the 80 KiB moderation frame
ceiling before allocation, incremental reads avoid length-prefix preallocation,
and a single shared inbound-byte budget prevents per-session multiplication of
the general 200 MiB ceiling. The injected broker-server library rejects
missing, extra, substituted, stale, or drifting backends.

The repository still supplies no genuine deployment-owned PKCS#11 or
managed-KMS backend and no operator credential. This blocker closes only when a
reviewed deployment injects that backend into the packaged broker boundary,
binds its public key handle to the reviewed deployment, and passes
provider-outage, config/runtime mismatch, wrong key/tag/AAD, chunk reorder,
range, restart recovery, rotation/rewrap, rollback, concurrency, and
payload-free log/API tests. A tests-only wrapper, local file key, environment
key, software wrapping fallback, or committed credential cannot satisfy the
gate.

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
finalized cursor/block-time pair, acquires bounded durable source claims,
verifies and ingests provider content, constructs the exact fee-quoted completion
transaction, submits through the queue, and reconciles committed completion or
cancellation. Its external sealed monotonic checkpoint authority, retention-only
terminal pruning, retry/dead-letter path, crash recovery, identity-pinned
runtime seams, blocking-work isolation, payload-free status, and worker-liveness
readiness fail closed. The configured source-pool, signer-resolver, and
checkpoint bindings contain only their stable handles, non-zero
adapter/public-policy revisions, and non-zero public policy digests. Enabled
startup requires all three providers and rejects missing, substituted, stale,
or test-marked providers. Storage-only nodes do not open the completion outbox
unless this worker is explicitly configured.

The standard authenticated source-pool coordinator now freezes a bounded
canonical inventory of at least two non-local governed provider identities and
distinct production handles. A finalized fetch list must be nonempty, strictly
provider-sorted, within the pool bound, and completely present before any I/O.
Each selected child source is identity/readiness checked before and after
fetch, and `irohad` requires the exact same payload-free pool qualification and
inventory after startup readiness, on every supervised tick, and around every
fetch. Endpoint, grant, credential, and payload material stays inside
runtime-only child adapters and is not copied into pool metadata, configuration,
or durable state. The completion-signer seam has independent public bindings
for the signer resolver and leaf HSM/KMS signer: the resolver has its own
revision and policy digest, while the signer retains its exact handle, adapter
revision, complete governed policy identity/digest lineage, admitted Ed25519 or
ML-DSA algorithm, and canonical public key. Startup, resolution, eligibility,
and pre/post-sign checks reject binding drift without exposing credentials or
private keys. This does not complete the blocker: deployment-owned
governance-advert/stream-grant/pinned-HTTPS child transports, a concrete HSM/KMS
signer backend with atomic rotation/revocation enforcement, and the external
sealed-CAS backend implementing the archive's now-wired retention-authority
contract remain missing.

`observe_finalized_snapshot(cursor, finalized_block_time_ms)` is the sole
durable writer of that cursor/time pair. Transaction rejection, finalized
completion or cancellation, and both absent-record reconciliation paths first
require their terminal evidence cursor to equal the retained snapshot. An
absent or different snapshot returns `StaleFinalizedCursor`; a half-populated
pair or zero retained block time returns `InvalidCheckpoint`. This validation
runs before record lookup, mutation, or an idempotent early return.

The external sealed head is authoritative. Its canonical record binds the
namespace/version, monotonic checkpoint sequence, predecessor CAS revision and
checkpoint digest, exact bounded checkpoint bytes and digest, and deterministic
content-addressed revision. Every load and CAS receives authoritative pre/post
provider identity and qualification checks. After a reported-success or
ambiguous CAS, exact-successor readback proves success and exact-predecessor
readback produces the explicit safe-retry `CheckpointCasUnchanged`; any other
head, unavailable readback, or provider drift is ambiguous and fails closed.
The no-follow atomic local file is a revalidated cache only: it may be absent,
match the head, or be exactly one predecessor behind it, and it never seeds or
overrides the external authority.

The repository now provides the bounded identity-pinned multi-provider
selection coordinator, but not concrete production governance-advert,
authenticated stream-grant, or pinned-HTTPS child transports, nor the
governance-aware HSM/KMS completion-signer resolver. The commit-owned capture
path scans bounded chain-authoritative state once per finalized anchor and
publishes a Kura-authenticated, provider-indexed immutable archive; runtime
exact-anchor pages no longer scan the live head. The archive also implements an
explicit generation-, key-, digest-, and Kura-finality-fenced prefix
compaction. Its read-only proposal binds the exact fence and canonical
checkpoint bytes; a separate per-chain sealed monotonic CAS record must be
authoritatively read back before the checkpoint is published or any prefix is
unlinked. Manual startup never auto-installs a checkpoint, and authority
startup recovers only the exact approved lineage across
CAS/publication/cleanup crash boundaries. Automatic age/capacity retention
policy remains deliberately disabled; the deployment must still supply the
external sealed-CAS backend.
The six-field `CompleteReplicationOrder` hard cut carries the exact expected
provider owner and four-part signer-policy chain, assignment revision, and
finalized anchor into the transaction. Ledger execution atomically revalidates
all three bindings against the same authoritative state used to commit the
completion, so owner reassignment, policy rotation/revocation, assignment
revision, and finalized-anchor changes cannot cross the check-to-commit
boundary. A newer finalized owner/policy
invalidates only `Signing` or provably never-exposed `Signed`; exposed bytes
stay on the reconciliation path before authority changes, including after
restart. Exact durable replays retain the accepted tuple; legacy or partial
completion material is rejected. The deployment resolver must still supply the
governed HSM/KMS identity and live owner/key/policy eligibility used to construct
that transaction.

Shutdown remains bounded only when the deployment source reader honors the
required deadline inside each underlying read; the wrapper cannot interrupt an
inner `Read::read` that blocks indefinitely. Readiness probes fail closed at
their configured timeout and stop the supervised worker, but a timed-out
`spawn_blocking` task cannot be force-cancelled and may linger until its
provider call returns. This blocker closes only when the deployment-owned
authenticated transport, signer resolver, and sealed-CAS retention backend are
implemented, focused/workspace and adversarial
outage/rotation/restart/capacity/deadline tests pass, and the reviewed
four-validator deployment proves one completion under simultaneous cross-peer
submission and recovery. A null/test adapter, unauthenticated or file-backed
source, local software/file/environment signing key, fabricated assignment
page, unbounded scan, or queue acceptance without committed reconciliation
cannot satisfy readiness.

### V1-BLOCK-REPUTATION-RUNTIME-01 — committed journal delivery and deployment adapters

The committed multi-feed projector and publication reconciler now have strict
non-secret configuration, runtime-only adapter injection, supervised
startup/shutdown, fail-closed monotonic freshness, payload-free status, and
bounded metrics. Standard `irohad` requires an externally authenticated
journal-transaction submitter and does not construct a queue-backed or
validator-key fallback. The committed projection is exposed to Torii only after
authenticated DAG readback and a fresh successful reconciliation. The
canonical V1 readback now carries a
pinned signed head plus a contiguous inclusion suffix bounded by the manifest
checkpoint window. It requires the exact threshold result once, validates every
CID, signature, parent, sequence, and timestamp through the head, links a new
suffix to the previously authenticated head, rejects rollback/fork/provider
qualification drift, and persists/reverifies the head and path on restart
before committed reads. Focused locked `sorafs_node` and `irohad` validation is
green for the prior slice; the latest compact-archive and launcher hardening is
pending focused validation. The current-head `State` query adapter was removed
because it cannot provide immutable historical exact-anchor pages; enabling the
runtime without its Kura-authenticated compact archive, external threshold
signer, or authenticated Governance DAG publication/readback adapter fails
startup.
The standard launcher now resolves the journal transaction submitter, threshold
signer, and Governance DAG roles through broker slots 32, 33, and 34. Each slot
is pinned by a required non-zero revision and lowercase 32-byte public policy
digest, is qualified before and after use, and poisons the broker session on a
mutating ambiguous result so a submission is never replayed automatically.
These bindings expose no credential, private key, or signed payload.

This blocker closes only when full workspace validation passes and
Kura-authenticated compact-archive capture/startup plus deployment-owned
`ReputationJournalTransactionSubmitterV1`,
`ReputationThresholdSignerClientV1`, and
`ReputationGovernanceDagClientV1` adapters are injected; slot 46 supplies a
genuine immutable finalized-PoR archive with an external Ed25519 HSM signer;
the stream-token owner invokes the durable journal callback; the complete
SDK/native validation matrix passes; the bounded exact retained-history
snapshot-id contract remains intact; identity/rotation/outage/
restart/fork/retry negatives pass; and the reviewed four-validator deployment
proves one committed outcome and recovery before promotion. A live current-head
view, local snapshot database, null adapter, file/environment credential,
fabricated ledger page, pre-finality submit receipt, or unsigned DAG
acknowledgement cannot satisfy readiness.

### V1-BLOCK-MODERATION-VIEWER-RUNTIME-01 — fenced orchestration and evidence authority

The native moderation ledger, fixed-view queries, bounded typed events,
finalized-chain orchestrator, exact transaction outbox, evidence authorization,
WebAuthn/grant flow, authenticated range decryption, signed hash-chained
receipts, an Ed25519-signed canonical checkpoint-digest/receipt-count/chain-head
anchor, a signed predecessor-bound external checkpoint-store record with
mandatory CAS readback and a revalidated local cache, exact checkpoint- and
predecessor-bound receipt projection, retention, legal holds, and erasure WAL
are present. `iroha_config`, the sanitized daemon registry, standard `irohad`,
and Torii carry the checkpoint store's non-secret handle, non-zero revision,
and non-zero public-policy digest to the service; enabled provider-less,
substituted, stale, unavailable, or test-marked startup fails closed. No
deployment-owned checkpoint-store implementation is supplied by this source
tree.

The moderation orchestrator also has a supervised payload-free
panel-notification delivery pass. It durably claims
each finalized-event-derived identity before calling an independently
qualified boundary, requires sink-side canonical-byte idempotency, and
checkpoints an exact stable receipt or bounded retry/dead-letter result.
Configuration and the standard daemon/Torii injection path bind that boundary
to one non-secret production handle, non-zero revision, and non-zero policy
digest; missing, substituted, stale, unavailable, or test-marked providers fail
closed.

Audit requests require the exact checkpoint digest and explicit digest-bound
page limit, accept one raw query ordering, return `409` on checkpoint change,
and reuse the retained signature without signer calls. Moderation GETs read
only a fresh worker-owned finalized projection. Maintenance is supervised with
deadline, non-overlap, monotonic-cursor, dead-letter, freshness, and liveness
fencing. The signed receipt checkpoint/projection is the sole evidence-access
audit authority; the old aggregate-audit POSTs are authenticated/authorized
`410 Gone` tombstones, and their scheduler and local registry calls are
removed. These source boundaries do not supply a deployment-owned messaging
provider or make the stock daemon a reference-production service. A valid
anchor proves integrity and signer identity, but a first-time consumer still
needs an independently authenticated monotonic public head to reject an older
validly signed anchor.

This blocker closes only when the reference deployment supplies real
runtime-only HSM/KMS/WebAuthn, messaging, settlement, publication, linearizable
sealed-CAS checkpoint-store, and authenticated downstream providers for every
enabled dependency; the viewer's shipped external-CAS path is exercised across
replicas while the orchestrator obtains equivalent predecessor-bound
single-writer fencing; the signed receipt projection is connected to a
transparency producer that anchors its monotonic checkpoint head for
first-contact freshness; semantic operation IDs are fenced across replicas;
and terminal receipts, operations, dead letters, holds, and erasure tombstones
have a signed replay-safe compaction/archive path. Same-height finalized hash
substitution, rollback, hung-provider, ambiguous delivery/CAS readback,
capacity, WebAuthn replay, IDOR, legal-hold race, restart, and multi-instance
takeover negatives must pass before four-validator deployment evidence can
close the lane.

### V1-BLOCK-POP-RUNTIME-01 — standard-daemon PoP provider adapter

The PoP issuer/wallet service, strict `iroha_config` policy, authenticated
Torii routes, and registry-qualified runtime injection seam are present. The
standard `irohad` entrypoint does not yet construct and inject a concrete
`PopCredentialRuntimeProviderRegistryV1`, and the repository does not contain
a deployable shared external-runtime or sidecar implementation that resolves
its enrollment/wallet hybrid secrets, governed issuer HSM, KMS key wrapper, API
authenticator, registry submitter/reader, private issuance/witness providers,
and finalized-time provider. Enabling PoP without that registry fails startup
by design.

This blocker closes only when the shared external-runtime lane packages the
registry implementation, binds its stable handle, exact non-zero policy
revision/digest, non-secret adapter handles, and public keys to the production
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
stable administrative identities. The strict optional
`[sorafs.por.potr_runtime]` configuration independently pins both signer
handles, identities, revisions and policy digests, the gateway public key,
distinct reader/source/resolver identities, and the complete non-zero baseline
finalized policy anchor. Enabled startup compares every public pin exactly
against the injected roles; configuration without roles, roles without enabled
configuration, partial or disabled-stale fields, test-marked/shared handles,
identity collisions, and substitutions fail closed. The provider
qualification is fixed to the baseline admission sequence/digest. The live
reader is queried before signing and again after both signatures; unavailable,
revoked, stale, identity-drifting, same-sequence-substituted, or mid-signature
changed policy fails closed. The tracker persists the exact policy
identity/digest/sequence, finalized height/hash, provider, and admission
envelope before any proof-outcome or repair handoff, then restores that binding
as the next query's monotonic floor. Torii no longer treats its startup
admission-registry snapshot alone as receipt authorization.

The authenticated reader path is concrete:
`PotrStateFinalizedPolicySourceV1` reads Torii's authoritative state and
`PotrFinalizedAdmissionReaderV1` combines that policy with the council-verified
admission registry. Configuration and Torii startup comparison are
source-complete, but focused/workspace Cargo validation remains pending. The
generic launcher intentionally supplies no gateway/provider signer roles.
This blocker closes only after focused/workspace Rust tests pass, the reference
deployment injects
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
match configuration. Any local signed-producer root additionally requires its
own exact config-pinned sealed monotonic CAS provider, including when the public
service is disabled. Producer-specific slots seal an empty-root identity before
the first append and an exact write-ahead intent for each successor. Recovery
reconciles that intent before the bounded full-root audit, which authenticates
canonical index, block, head, source payload, sidecar, signature, CID, lineage,
and reverse-map state. The supervised service separately requires injected
rotation-aware endpoint authenticators and sealed state. Missing, mismatched,
drifting, malformed, replayed, rolled-back, or unavailable providers fail
closed with provider diagnostics redacted.

The V1 wire now separates canonical binary and mutable-state ceilings. Source
payloads stop at 64 MiB; all node/block/head signing and CID inputs stop at
128 MiB; complete header-bearing blocks add one checked 64 KiB
signature/envelope allowance. Producer preflight exercises the parent-bearing
node, block, and CID shapes before source, latest, or JSON mutation. Block and
source readers use those distinct binary bounds while JSON indexes, queues, and
heads retain the independent 64 MiB mutable-state cap. The public-service
request default and minimum cover the exact block ceiling. Boundary tests do
not allocate objects near those maxima. Per-variant semantic collection/string
limits are enforced before nested validation. A borrowed non-cloning sizing
walk for the remaining bounded payload/node clones remains tracked local
hardening; these cap changes do not claim allocation-bomb closure or external
readiness.

The provider-qualification protocol gap is closed locally. Implicit rotation
still fails closed, while the explicit boundary requires independent old/new
signatures over a canonical key-transition envelope. Its monotonic
outgoing/incoming segment revisions and transition-body digest bind the
canonical root, exact sealed predecessor revision, current head and
predecessor/successor index digests, both publisher identities and Ed25519 keys,
both signer/store handle-revision-policy bindings, and monotonic
transition/archive heads. The producer checkpoint advances separate block,
transition, and archive generations. The live journal and each signed archive
are capped at 64 transitions, the archive chain is capped at 64 entries, and
archive plus sealed-checkpoint readback occurs before prune. Recovery rebuilds
that bounded lineage, validates each retained block under the authority active
at its sequence, and binds the signed head to the tip segment. It also completes
both a staged pre-CAS archive and a post-CAS/pre-prune archive idempotently;
canonical validation rejects tamper, fork, duplicate, replay, revision rollback,
truncation, trailing bytes, and qualification or key substitution. This is not
evidence of a genuine HSM or sealed-store deployment.

The runtime DAG block index remains full-history state. Qualification-history
compaction does not prune signed DAG blocks; that still requires a bounded
authenticated retained-prefix protocol. The producer intent now seals only
exact descriptors for a durably staged full index under a 64 KiB ceiling.
Genuine external provider deployment and long-running block-retention closure
must complete before the 24-hour qualification can be claimed.
On the filesystem-flag-qualified Linux, Android, macOS, iOS, FreeBSD, OpenBSD,
NetBSD, and DragonFly targets, producer/service roots and ancestors now have
role-specific owner/mode and trusted-sticky-parent policy, exact canonical
lexical paths, retained `O_DIRECTORY|O_NOFOLLOW` handles, and
device/inode/owner/mode/effective-UID revalidation around producer, source,
state, and mirror operations. Other Unix targets, and Android architectures
outside arm, aarch64, x86, x86_64, and riscv64, fail compilation until their
native flags and target tests are qualified. Linux/macOS/Windows descendant
operations are component-rooted through retained no-follow handles with exact
identity rechecks. Linux and macOS require two identical bounded descriptor ACL
snapshots and reject untrusted mutation grants or protected ACL namespaces.
Windows pins the root owner SID, strictly parses two identical bounded security
descriptors, rejects untrusted mutation grants, and retains file IDs through
crash recovery and atomic-temp cleanup. macOS qualification must use physical
canonical paths such as `/private/var/...`. Focused Rust validation of the
latest producer transaction and filesystem hardening is still pending.

The stock Linux/macOS daemon now supplies a client registry for the implemented
Governance DAG signer/store/request-authenticator, moderation-quarantine,
provider-ingest, and evidence-viewer roles through a platform-fixed
service-UID-owned local broker. The authenticated client and injected
server-library protocol are bounded and canonical and bind the chain, catalog,
session, qualification metadata, operation payload, and monotonic request
identity; unsupported roles and platforms fail closed. Governance IPFS/head
authentication uses a canonical descriptor and signed envelope with
configuration-pinned Ed25519 verification keys, bounded body/lifetime/skew,
replay rejection, and exact provider requalification. Credential headers,
private material, and compatibility forms cannot cross the boundary. The
repository does not bundle a deployment-owned broker executable or genuine
HSM, sealed-store, Kubo/head, or other deployment backends.

After the remaining local work, the blocker closes only when supported bundles
package those deployment-owned components, inject HSM-backed canonical-request
signers for Kubo/head operations, and supervise two instances with successful
CAS/failover, credential rotation, signed compaction/recovery, rollback,
checkpoint corruption, provider outage, public-mirror, and disaster-recovery
rehearsals on the reviewed deployment.

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
